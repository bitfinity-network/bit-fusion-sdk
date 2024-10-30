use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bitcoin::{Address, Amount};
use bridge_client::BridgeCanisterClient as _;
use bridge_did::brc20_info::Brc20Tick;
use bridge_did::id256::Id256;
use bridge_did::operations::{Brc20BridgeDepositOp, Brc20BridgeOp, Brc20BridgeWithdrawOp};
use rand::seq::SliceRandom;
use rand::Rng;

use super::{BaseTokens, BurnInfo, OwnedWallet, StressTestConfig, StressTestState, User};
use crate::context::brc20::{self as ctx, Brc20Context, Brc20InitArgs};
use crate::context::TestContext;
use crate::utils::error::{Result, TestError};
use crate::utils::token_amount::TokenAmount;

static USER_COUNTER: AtomicU32 = AtomicU32::new(0);

struct Brc20Token {
    tick: Brc20Tick,
    decimals: u8,
}

pub struct Brc20BaseTokens {
    ctx: Arc<Brc20Context>,
    tokens: Vec<Brc20Token>,
    ticks: Vec<Brc20Tick>,
}

impl Brc20BaseTokens {
    async fn init(base_tokens_number: usize) -> Result<Self> {
        println!("Creating brc20 token canisters");

        let mut brc20_to_deploy = Vec::with_capacity(base_tokens_number);
        for _ in 0..base_tokens_number {
            let mut rng = rand::thread_rng();
            let decimals = if rng.gen_bool(0.5) {
                None
            } else {
                Some(rng.gen_range(2..18))
            };

            let limit = if rng.gen_bool(0.5) {
                None
            } else {
                Some(
                    [1u64, 10, 100, 1_000, 10_000, 100_000]
                        .choose(&mut rng)
                        .copied()
                        .unwrap(),
                )
            };

            println!(
                "Creating BRC20 token canister with decimals: {:?}, limit: {:?}",
                decimals, limit
            );
            brc20_to_deploy.push(Brc20InitArgs {
                tick: ctx::generate_brc20_tick(),
                decimals,
                limit,
                max_supply: rng.gen_range(1_000_000..1_000_000_000),
            });
        }

        let brc20_context = Brc20Context::new(&brc20_to_deploy).await;
        let tokens = brc20_to_deploy
            .iter()
            .map(|args| Brc20Token {
                tick: args.tick,
                decimals: args.decimals.unwrap_or_default(),
            })
            .collect();
        let ticks = brc20_to_deploy.iter().map(|args| args.tick).collect();

        println!("BRC20 token canisters created");

        Ok(Self {
            ctx: Arc::new(brc20_context),
            tokens,
            ticks,
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
}

impl BaseTokens for Brc20BaseTokens {
    type TokenId = Brc20Tick;
    type UserId = Address;

    fn ctx(&self) -> &(impl TestContext + Send + Sync) {
        &self.ctx.inner
    }

    fn ids(&self) -> &[Self::TokenId] {
        &self.ticks
    }

    fn user_id256(&self, user_id: Self::UserId) -> Id256 {
        // encode address to 32 bytes
        let mut bytes = [0u8; 32];
        let addr_as_str = user_id.to_string();
        let addr_as_bytes = addr_as_str.as_bytes();

        if addr_as_bytes.len() > bytes.len() {
            panic!("Address is too long");
        }

        bytes[..addr_as_bytes.len()].copy_from_slice(addr_as_bytes);

        Id256(bytes)
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
        let address = ctx::generate_btc_wallet().address;
        // mint some tokens
        self.ctx
            .send_btc(&address, Amount::from_sat(10_000_000))
            .await
            .map_err(|e| TestError::Generic(e.to_string()))?;

        USER_COUNTER.fetch_add(1, Ordering::Relaxed);

        Ok(address)
    }

    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: did::U256) -> Result<()> {
        let token = self.token_info(token_idx)?;
        let amount: u128 = amount.0.as_u128();

        let amount = TokenAmount::from_decimals(amount, token.decimals);
        self.ctx
            .withdraw(to, &token.tick, amount)
            .await
            .map_err(|e| TestError::Generic(e.to_string()))
    }

    async fn balance_of(&self, token_idx: usize, user: &Self::UserId) -> Result<did::U256> {
        let token = self.tick(token_idx)?;

        self.ctx
            .brc20_balance(user, token)
            .await
            .map_err(|e| TestError::Generic(e.to_string()))
            .map(|amount| amount.amount().into())
    }

    async fn deposit(
        &self,
        to_user: &User<Self::UserId>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<did::U256> {
        let token = self.token_info(info.base_token_idx)?;
        let nonce = to_user.next_nonce();

        let tick = token.tick;
        let amount = TokenAmount::from_decimals(info.amount.0.as_u128(), token.decimals);
        let dst_address = &info.wrapped_token;

        self.ctx
            .deposit(tick, amount, dst_address, &to_user.wallet, nonce.into())
            .await?;

        Ok(info.amount.clone())
    }

    async fn set_bft_bridge_contract_address(&self, bft_bridge: &did::H160) -> Result<()> {
        self.ctx()
            .brc20_bridge_client(self.ctx().admin_name())
            .set_bft_bridge_contract(bft_bridge)
            .await?;

        Ok(())
    }

    async fn is_operation_complete(
        &self,
        address: did::H160,
        memo: bridge_did::operation_log::Memo,
    ) -> Result<bool> {
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
