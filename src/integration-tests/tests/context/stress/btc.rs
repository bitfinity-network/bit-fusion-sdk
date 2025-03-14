use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bitcoin::Amount;
use bridge_client::BridgeCanisterClient as _;
use bridge_did::id256::Id256;
use bridge_did::operations::BtcBridgeOp;
use did::H160;
use eth_signer::LocalWallet;
use tokio::sync::RwLock;

use super::{BaseTokens, BurnInfo, OwnedWallet, StressTestConfig, StressTestState, User};
use crate::context::btc_bridge::BtcContext;
use crate::context::TestContext;
use crate::utils::btc_wallet::BtcWallet;
use crate::utils::error::{Result, TestError};

const FEE: u128 = 3_000;

type AsyncMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

struct UserWallet {
    btc_wallet: BtcWallet,
    wallet: LocalWallet,
}

impl UserWallet {
    async fn new(wallet: LocalWallet) -> Self {
        let btc_wallet = BtcWallet::new_random();

        Self { btc_wallet, wallet }
    }
}

pub struct BtcToken<Ctx>
where
    Ctx: TestContext + Sync,
{
    ctx: Arc<BtcContext<Ctx>>,
    max_supply: Amount,
    users: AsyncMap<H160, Arc<UserWallet>>,
    token_ids: Vec<Id256>,
}

impl<Ctx> BtcToken<Ctx>
where
    Ctx: TestContext + Sync,
{
    async fn init(ctx: Ctx) -> Result<Self> {
        println!("Creating BTC token canisters");

        let btc_context = BtcContext::new(ctx).await;

        println!("BTC token canisters created");
        let token_ids = vec![btc_context.ckbtc_ledger_id];

        Ok(Self {
            ctx: Arc::new(btc_context),
            max_supply: Amount::from_int_btc(21_000_000),
            users: AsyncMap::default(),
            token_ids,
        })
    }

    async fn user_wallet(&self, user: &H160) -> Result<Arc<UserWallet>> {
        self.users
            .read()
            .await
            .get(user)
            .cloned()
            .ok_or(TestError::Generic("User not found".to_string()))
    }
}

impl<Ctx> BaseTokens for BtcToken<Ctx>
where
    Ctx: TestContext + Send + Sync,
{
    type TokenId = Id256;
    type UserId = H160;

    fn ctx(&self) -> &(impl TestContext + Send + Sync) {
        &self.ctx.context
    }

    fn ids(&self) -> &[Self::TokenId] {
        &self.token_ids
    }

    async fn user_id(&self, user_id: Self::UserId) -> Vec<u8> {
        self.user_wallet(&user_id)
            .await
            .unwrap()
            .btc_wallet
            .address
            .to_string()
            .as_bytes()
            .to_vec()
    }

    fn token_id256(&self, token_id: Self::TokenId) -> Id256 {
        token_id
    }

    async fn bridge_canister_evm_address(&self) -> Result<did::H160> {
        let client = self.ctx().btc_bridge_client(self.ctx().admin_name());
        let address = client.get_bridge_canister_evm_address().await??;
        Ok(address)
    }

    async fn new_user(&self, wrapped_wallet: &OwnedWallet) -> Result<Self::UserId> {
        // generate random address
        let wallet = UserWallet::new(wrapped_wallet.clone()).await;

        let user_id: H160 = wallet.wallet.address().into();

        self.users
            .write()
            .await
            .insert(user_id.clone(), Arc::new(wallet));

        Ok(user_id)
    }

    async fn mint(&self, _token_idx: usize, to: &Self::UserId, amount: did::U256) -> Result<()> {
        let amount = amount.0.to();
        if self.max_supply.to_sat() < amount {
            return Err(TestError::Generic("Mint amount exceeds max supply".into()));
        }

        let user_wallet = self.user_wallet(to).await?;
        let to = &user_wallet.btc_wallet.address;
        self.ctx
            .get_funding_utxo(to, Amount::from_sat(amount))
            .await
            .map_err(|e| TestError::Generic(e.to_string()))?;

        Ok(())
    }

    async fn balance_of(&self, _token_idx: usize, user: &Self::UserId) -> Result<did::U256> {
        let address = self.user_wallet(user).await?.btc_wallet.address.clone();
        let balance = self.ctx.btc_balance(&address).await;

        Ok(did::U256::from(balance.to_sat()))
    }

    async fn deposit(
        &self,
        to_user: &User<Self::UserId>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<did::U256> {
        let amount = info.amount.0.to();
        if self.max_supply.to_sat() < amount {
            return Err(TestError::Generic("Mint amount exceeds max supply".into()));
        }

        // convert amount to satoshi
        let amount = Amount::from_sat(amount);
        let nonce = to_user.next_nonce();

        let from_user = self.user_wallet(&info.from).await?;

        // wait for mint

        let recipient_wallet = self.user_wallet(&to_user.base_id).await?;
        let to = recipient_wallet.wallet.address().into();

        let prev_balance = self
            .ctx
            .erc20_balance_of(&recipient_wallet.wallet, Some(&to))
            .await
            .expect("Failed to get balance");

        // mint tokens
        println!(
            "Minting {amount} tokens from {} to {to} with memo {:?}",
            from_user.wallet.address(),
            info.memo,
        );

        self.ctx
            .deposit_btc(
                &from_user.wallet,
                &from_user.btc_wallet,
                amount,
                &to,
                info.memo,
                nonce,
            )
            .await
            .expect("Mint failed");

        let start = std::time::Instant::now();

        println!("Expected to have balance on {to}");
        let new_balance = loop {
            let balance = self
                .ctx
                .erc20_balance_of(&recipient_wallet.wallet, Some(&to))
                .await
                .expect("Failed to get balance");

            if balance > prev_balance {
                break balance;
            }

            if start.elapsed() > Duration::from_secs(60) {
                return Err(TestError::Generic("Mint timeout".into()));
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        };

        println!("new balance: {new_balance}; prev balance: {prev_balance}");

        let diff = new_balance - prev_balance;

        assert_eq!(diff, amount.to_sat() as u128 - FEE);

        Ok(info.amount.clone())
    }

    async fn before_withdraw(
        &self,
        token_idx: usize,
        user_id: Self::UserId,
        user_wallet: &OwnedWallet,
        amount: did::U256,
    ) -> Result<()> {
        println!(
            "Before withdraw for token {token_idx} and user {user_id}; user {}; amount {}",
            user_wallet.address(),
            amount
        );
        println!(
            "Before wallet: base token should be {:?}",
            self.ctx.ckbtc_ledger_id.0
        );
        Ok(())
    }

    async fn set_btf_bridge_contract_address(&self, btf_bridge: &did::H160) -> Result<()> {
        println!("Setting btf bridge contract address: {btf_bridge}");
        self.ctx
            .set_btf_bridge_contract(btf_bridge)
            .await
            .map_err(|e| TestError::Generic(e.to_string()))?;

        Ok(())
    }

    async fn create_wrapped_token(
        &self,
        admin_wallet: &OwnedWallet,
        _btf_bridge: &did::H160,
        _token_id: Id256,
    ) -> Result<did::H160> {
        self.ctx
            .create_wrapped_token(admin_wallet)
            .await
            .map_err(|e| TestError::Generic(e.to_string()))
    }

    async fn is_operation_complete(
        &self,
        address: did::H160,
        memo: bridge_did::operation_log::Memo,
    ) -> Result<bool> {
        println!("getting operation by memo: {memo:?} and user: {address}");
        let op_info = self
            .ctx()
            .btc_bridge_client(self.ctx().admin_name())
            .get_operation_by_memo_and_user(memo, &address)
            .await?;

        let op = match op_info {
            Some((_, op)) => op,
            None => {
                return Err(TestError::Generic("operation not found".into()));
            }
        };

        Ok(matches!(
            op,
            BtcBridgeOp::Erc20MintConfirmed(_) | BtcBridgeOp::BtcWithdrawConfirmed { .. }
        ))
    }
}

/// Run stress test with the given TestContext implementation.
pub async fn stress_test_btc_bridge_with_ctx<Ctx>(ctx: Ctx, config: StressTestConfig)
where
    Ctx: TestContext + Send + Sync,
{
    let base_tokens = BtcToken::init(ctx).await.expect("failed to init BtcToken");

    // timeout
    let stress_test_stats = StressTestState::run(&base_tokens, config)
        .await
        .expect("test failed");

    base_tokens.ctx.stop().await;

    dbg!(&stress_test_stats);

    assert_eq!(stress_test_stats.failed_roundtrips, 0);

    // TODO: fix, fee is currently not supporting BTC address <https://infinityswap.atlassian.net/browse/EPROD-1062>
    //assert!(
    //    stress_test_stats.init_bridge_canister_native_balance
    //        <= stress_test_stats.finish_bridge_canister_native_balance
    //);
}
