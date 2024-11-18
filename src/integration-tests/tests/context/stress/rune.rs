use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bitcoin::{Address, Amount};
use bridge_client::BridgeCanisterClient as _;
use bridge_did::id256::{Id256, ID_256_BYTE_SIZE};
use bridge_did::operations::{RuneBridgeDepositOp, RuneBridgeOp, RuneBridgeWithdrawOp};
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use eth_signer::Signer as _;
use ordinals::RuneId;

use super::{BaseTokens, BurnInfo, OwnedWallet, StressTestConfig, StressTestState, User};
use crate::context::rune::{self as ctx, BtcWallet, RunesContext};
use crate::context::TestContext;
use crate::utils::error::{Result, TestError};

static USER_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct RuneBaseTokens<Ctx>
where
    Ctx: TestContext + Sync,
{
    ctx: Arc<RunesContext<Ctx>>,
    tokens: Vec<RuneId>,
    users: DashMap<Id256, BtcWallet>,
}

impl<Ctx> RuneBaseTokens<Ctx>
where
    Ctx: TestContext + Sync,
{
    async fn init(ctx: Ctx, base_tokens_number: usize) -> Result<Self> {
        println!("Creating rune token canisters");

        let mut runes_to_deploy = Vec::with_capacity(base_tokens_number);
        for _ in 0..base_tokens_number {
            let name = ctx::generate_rune_name();
            println!("Creating Rune {name}",);
            runes_to_deploy.push(name);
        }

        let rune_context = RunesContext::new(ctx, &runes_to_deploy).await;

        println!("Rune token canisters created");

        Ok(Self {
            tokens: rune_context.runes.runes.keys().copied().collect(),
            ctx: Arc::new(rune_context),
            users: DashMap::new(),
        })
    }

    fn token_info(&self, idx: usize) -> Result<&RuneId> {
        self.tokens
            .get(idx)
            .ok_or(TestError::Generic("Token not found".to_string()))
    }

    /// Get the rune by index
    fn rune(&self, idx: usize) -> Result<RuneId> {
        self.tokens
            .get(idx)
            .map(|token| token)
            .copied()
            .ok_or(TestError::Generic("Token not found".to_string()))
    }

    fn user_wallet(&self, user: &Id256) -> Result<Ref<'_, Id256, BtcWallet>> {
        self.users
            .get(user)
            .ok_or(TestError::Generic("User not found".to_string()))
    }

    async fn rune_balance(&self, address: &Address, rune: &RuneId) -> Result<u128> {
        self.ctx
            .ord_rune_balance(address, rune)
            .await
            .map_err(|e| TestError::Generic(e.to_string()))
    }
}

impl<Ctx> BaseTokens for RuneBaseTokens<Ctx>
where
    Ctx: TestContext + Send + Sync,
{
    type TokenId = RuneId;
    type UserId = Id256;

    fn ctx(&self) -> &(impl TestContext + Send + Sync) {
        &self.ctx.inner
    }

    fn ids(&self) -> &[Self::TokenId] {
        &self.tokens
    }

    fn user_id(&self, user_id: Self::UserId) -> Vec<u8> {
        self.user_wallet(&user_id)
            .unwrap()
            .address
            .to_string()
            .as_bytes()
            .to_vec()
    }

    fn token_id256(&self, token_id: Self::TokenId) -> Id256 {
        Id256::from_btc_tx_index(token_id.block, token_id.tx)
    }

    async fn bridge_canister_evm_address(&self) -> Result<did::H160> {
        let client = self.ctx().rune_bridge_client(self.ctx().admin_name());
        let address = client.get_bridge_canister_evm_address().await??;
        Ok(address)
    }

    async fn new_user(&self, _wrapped_wallet: &OwnedWallet) -> Result<Self::UserId> {
        // generate random address
        let wallet = ctx::generate_btc_wallet();
        // mint some tokens
        self.ctx
            .send_btc(&wallet.address, Amount::from_sat(10_000_000))
            .await
            .map_err(|e| TestError::Generic(e.to_string()))?;
        let next_id = USER_COUNTER.fetch_add(1, Ordering::Relaxed);

        let mut id_buf = [0u8; ID_256_BYTE_SIZE];
        id_buf[..4].copy_from_slice(&next_id.to_be_bytes());
        let id = Id256(id_buf);

        self.users.insert(id, wallet);

        Ok(id)
    }

    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: did::U256) -> Result<()> {
        let rune = self.token_info(token_idx)?;
        let amount: u128 = amount.0.as_u128();

        let to = self.user_wallet(to)?.address.clone();

        // mint tokens
        println!("Minting {amount} of {rune} tokens to {to}");

        self.ctx
            .send_runes(&self.ctx.runes.ord_wallet, &to, &[(rune, amount)])
            .await
            .map_err(|e| TestError::Generic(e.to_string()))?;

        // wait for mint
        self.ctx.wait_for_blocks(6).await;

        // verify balance
        let balance = self.rune_balance(&to, &rune).await?;

        println!("current balance amount: {balance}",);
        println!("expected balance amount: {amount}",);

        assert_eq!(balance, amount);

        Ok(())
    }

    async fn balance_of(&self, token_idx: usize, user: &Self::UserId) -> Result<did::U256> {
        let rune = self.rune(token_idx)?;
        let user = self.user_wallet(user)?.address.clone();

        let balance = self
            .rune_balance(&user, &rune)
            .await
            .map_err(|e| TestError::Generic(e.to_string()))?;

        Ok(did::U256::from(balance))
    }

    async fn deposit(
        &self,
        to_user: &User<Self::UserId>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<did::U256> {
        let rune = self.token_info(info.base_token_idx)?;
        let nonce = to_user.next_nonce();

        // transfer Rune first
        let from_user = self.user_wallet(&info.from)?;
        println!("deposit from user: {}", from_user.address);

        // check balance
        let balance = self.rune_balance(&from_user.address, &rune).await?;
        let amount = info.amount.0.as_u128();
        if balance < amount {
            return Err(TestError::Generic(
                "Can't deposit: Insufficient RUNE balance".into(),
            ));
        }

        let eth_wallet_address = to_user.address();
        let deposit_address = self.ctx.get_deposit_address(&eth_wallet_address).await;
        println!(
            "Sending RUNE from {from} with ETH address: {eth_wallet_address} to deposit address: {deposit_address}, tick: {rune}, amount: {amount}",
            from = from_user.address,
        );
        self.ctx
            .send_runes(&from_user, &deposit_address, &[(rune, amount)])
            .await
            .expect("send RUNE failed");

        self.ctx
            .deposit(
                &[*rune],
                None,
                &eth_wallet_address,
                &to_user.wallet,
                nonce.into(),
                Some(info.memo),
            )
            .await?;

        Ok(info.amount.clone())
    }

    async fn before_withdraw(
        &self,
        _token_idx: usize,
        _user_id: Self::UserId,
        user_wallet: &OwnedWallet,
        _amount: did::U256,
    ) -> Result<()> {
        let deposit_address = self
            .ctx
            .get_deposit_address(&user_wallet.address().into())
            .await;

        println!("before withdraw: sending BTC to deposit address: {deposit_address}");

        self.ctx
            .send_btc(&deposit_address, Amount::from_sat(100_000_000))
            .await
            .map_err(|e| TestError::Generic(e.to_string()))?;

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
        token_id: Id256,
    ) -> Result<did::H160> {
        let (block, tx) = token_id
            .to_btc_tx_index()
            .map_err(|_| TestError::Generic("Invalid token id".to_string()))?;
        let tick = RuneId { block, tx };

        println!("Creating wrapped token with tick: {tick}");

        self.ctx
            .create_wrapped_token(admin_wallet, tick)
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
            .rune_bridge_client(self.ctx().admin_name())
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
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::MintOrderConfirmed { .. })
                | RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::TransactionSent { .. })
        ))
    }
}

/// Run stress test with the given TestContext implementation.
pub async fn stress_test_rune_bridge_with_ctx<Ctx>(
    ctx: Ctx,
    base_tokens_number: usize,
    config: StressTestConfig,
) where
    Ctx: TestContext + Send + Sync,
{
    let base_tokens = RuneBaseTokens::init(ctx, base_tokens_number).await.unwrap();
    let stress_test_stats = StressTestState::run(&base_tokens, config).await.unwrap();

    base_tokens.ctx.stop().await;

    dbg!(&stress_test_stats);

    assert_eq!(stress_test_stats.failed_roundtrips, 0);

    // TODO: fix, fee is currently not supporting BTC address <https://infinityswap.atlassian.net/browse/EPROD-1062>
    //assert!(
    //    stress_test_stats.init_bridge_canister_native_balance
    //        <= stress_test_stats.finish_bridge_canister_native_balance
    //);
}
