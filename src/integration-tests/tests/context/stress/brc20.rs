use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bitcoin::{Address, Amount};
use bridge_client::BridgeCanisterClient as _;
use bridge_did::brc20_info::Brc20Tick;
use bridge_did::id256::{Id256, ID_256_BYTE_SIZE};
use bridge_did::operations::{Brc20BridgeDepositOp, Brc20BridgeOp, Brc20BridgeWithdrawOp};
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use eth_signer::Signer as _;
use rand::seq::SliceRandom;
use rand::Rng;

use super::{BaseTokens, BurnInfo, OwnedWallet, StressTestConfig, StressTestState, User};
use crate::context::brc20::{self as ctx, Brc20Context, Brc20InitArgs, BtcWallet};
use crate::context::TestContext;
use crate::utils::error::{Result, TestError};
use crate::utils::token_amount::TokenAmount;

static USER_COUNTER: AtomicU32 = AtomicU32::new(0);

struct Brc20Token {
    tick: Brc20Tick,
    decimals: u8,
    max_supply: u64,
}

pub struct Brc20BaseTokens<Ctx>
where
    Ctx: TestContext + Sync,
{
    ctx: Arc<Brc20Context<Ctx>>,
    tokens: Vec<Brc20Token>,
    ticks: Vec<Brc20Tick>,
    users: DashMap<Id256, BtcWallet>,
}

impl<Ctx> Brc20BaseTokens<Ctx>
where
    Ctx: TestContext + Sync,
{
    async fn init(ctx: Ctx, base_tokens_number: usize) -> Result<Self> {
        println!("Creating brc20 token canisters");

        let mut brc20_to_deploy = Vec::with_capacity(base_tokens_number);
        for _ in 0..base_tokens_number {
            let mut rng = rand::thread_rng();
            let decimals = Some(rng.gen_range(2..18));

            println!("Creating BRC20 token canister with decimals: {decimals:?}",);
            brc20_to_deploy.push(Brc20InitArgs {
                tick: ctx::generate_brc20_tick(),
                decimals,
                limit: None,
                max_supply: [
                    100_000_000u64,
                    1_000_000_000,
                    10_000_000_000,
                    100_000_000_000,
                ]
                .choose(&mut rng)
                .copied()
                .unwrap(),
            });
        }

        let brc20_context = Brc20Context::new(ctx, &brc20_to_deploy).await;
        let tokens = brc20_to_deploy
            .iter()
            .map(|args| Brc20Token {
                tick: args.tick,
                decimals: args.decimals.unwrap_or_default(),
                max_supply: args.max_supply,
            })
            .collect();
        let ticks = brc20_to_deploy.iter().map(|args| args.tick).collect();

        println!("BRC20 token canisters created");

        Ok(Self {
            ctx: Arc::new(brc20_context),
            tokens,
            ticks,
            users: DashMap::new(),
        })
    }

    fn token_info(&self, idx: usize) -> Result<&Brc20Token> {
        self.tokens
            .get(idx)
            .ok_or(TestError::Generic("Token not found".to_string()))
    }

    /// Get the token by index
    fn tick(&self, idx: usize) -> Result<&Brc20Tick> {
        self.tokens
            .get(idx)
            .map(|token| &token.tick)
            .ok_or(TestError::Generic("Token not found".to_string()))
    }

    fn user_wallet(&self, user: &Id256) -> Result<Ref<'_, Id256, BtcWallet>> {
        self.users
            .get(user)
            .ok_or(TestError::Generic("User not found".to_string()))
    }

    async fn brc20_balance(&self, address: &Address, tick: &Brc20Tick) -> Result<TokenAmount> {
        self.ctx
            .brc20_balance(address, tick)
            .await
            .map_err(|e| TestError::Generic(e.to_string()))
    }
}

impl<Ctx> BaseTokens for Brc20BaseTokens<Ctx>
where
    Ctx: TestContext + Send + Sync,
{
    type TokenId = Brc20Tick;
    type UserId = Id256;

    fn ctx(&self) -> &(impl TestContext + Send + Sync) {
        &self.ctx.inner
    }

    fn ids(&self) -> &[Self::TokenId] {
        &self.ticks
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
        Id256::from_brc20_tick(token_id.inner())
    }

    async fn bridge_canister_evm_address(&self) -> Result<did::H160> {
        let client = self.ctx().brc20_bridge_client(self.ctx().admin_name());
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
        let token = self.token_info(token_idx)?;
        let amount: u128 = amount.0.as_u128();
        if (token.max_supply as u128) < amount {
            return Err(TestError::Generic("Mint amount exceeds max supply".into()));
        }

        let to = self.user_wallet(to)?.address.clone();

        let amount = TokenAmount::from_int(amount, token.decimals);
        // mint tokens
        println!("Minting {amount} tokens to {to}");
        self.ctx
            .mint(token.tick, amount, &to)
            .await
            .map_err(|e| TestError::Generic(e.to_string()))?;

        // wait for mint
        self.ctx.wait_for_blocks(6).await;

        // verify balance
        let balance = self.brc20_balance(&to, &token.tick).await?;

        println!(
            "current balance amount: {}; as int: {}; decimals: {}",
            balance.amount(),
            balance.as_int(),
            balance.decimals()
        );
        println!(
            "expected balance amount: {}; as int: {}; decimals: {}",
            amount.amount(),
            amount.as_int(),
            amount.decimals()
        );

        assert_eq!(balance.as_int(), amount.as_int());

        Ok(())
    }

    async fn balance_of(&self, token_idx: usize, user: &Self::UserId) -> Result<did::U256> {
        let token = self.tick(token_idx)?;
        let user = self.user_wallet(user)?.address.clone();

        self.brc20_balance(&user, token)
            .await
            .map(|amount| amount.as_int().into())
    }

    async fn deposit(
        &self,
        to_user: &User<Self::UserId>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<did::U256> {
        let token = self.token_info(info.base_token_idx)?;
        let nonce = to_user.next_nonce();

        let tick = token.tick;
        let amount = TokenAmount::from_int(info.amount.0.as_u128(), token.decimals);

        // transfer BRC20 first
        let from_user = self.user_wallet(&info.from)?;
        println!("deposit from user: {}", from_user.address);

        // check balance
        let balance = self.brc20_balance(&from_user.address, &tick).await?;
        if balance.as_int() < amount.as_int() {
            return Err(TestError::Generic(
                "Can't deposit: Insufficient BRC20 balance".into(),
            ));
        }

        let eth_wallet_address = to_user.address();
        let deposit_address = self.ctx.get_deposit_address(&eth_wallet_address).await;
        println!(
            "Sending BRC20 from {from} with ETH address: {eth_wallet_address} to deposit address: {deposit_address}, tick: {tick}, amount: {amount}",
            from = from_user.address,
        );
        self.ctx
            .send_brc20(&from_user, &deposit_address, tick, amount)
            .await
            .expect("send brc20 failed");

        self.ctx
            .deposit(
                tick,
                amount,
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
            .map_err(|e| TestError::Generic(e.to_string()))
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
        let tick = Brc20Tick::from(token_id);
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
            .brc20_bridge_client(self.ctx().admin_name())
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
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { .. })
                | Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { .. })
        ))
    }
}

/// Run stress test with the given TestContext implementation.
pub async fn stress_test_brc20_bridge_with_ctx<Ctx>(
    ctx: Ctx,
    base_tokens_number: usize,
    config: StressTestConfig,
) where
    Ctx: TestContext + Send + Sync,
{
    let base_tokens = Brc20BaseTokens::init(ctx, base_tokens_number)
        .await
        .unwrap();
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
