use alloy_sol_types::SolCall;
use bridge_client::BridgeCanisterClient;
use bridge_did::id256::Id256;
use did::{H160, U256, U64};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;

use super::{BaseTokens, BurnInfo, StressTestConfig, StressTestState};
use crate::context::TestContext;
use crate::dfx_tests::TestWTM::{self};
use crate::utils::error::Result;
use crate::utils::EXTERNAL_EVM_CHAIN_ID;

static MEMO_COUNTER: AtomicU32 = AtomicU32::new(0);

type OwnedWallet = Wallet<'static, SigningKey>;

pub struct Erc20BaseTokens<Ctx> {
    ctx: Ctx,
    tokens: Vec<H160>,
    contracts_deployer: OwnedWallet,
    bft_bridge: H160,
    users: RwLock<HashMap<H160, OwnedWallet>>,
}

impl<Ctx: TestContext + Send + Sync> Erc20BaseTokens<Ctx> {
    async fn init(ctx: Ctx, base_tokens_number: usize) -> Result<Self> {
        let external_evm_client = ctx.external_evm_client(ctx.admin_name());
        let contracts_deployer = Wallet::new(&mut rand::thread_rng());
        let deployer_address = contracts_deployer.address();
        external_evm_client
            .mint_native_tokens(deployer_address.into(), u128::MAX.into())
            .await
            .unwrap()
            .unwrap();

        let mut tokens = Vec::with_capacity(base_tokens_number);
        for _ in 0..base_tokens_number {
            let icrc_principal = ctx
                .deploy_test_wtm_token_on_evm(
                    &external_evm_client,
                    &contracts_deployer,
                    u128::MAX.into(),
                )
                .await
                .unwrap();
            tokens.push(icrc_principal);
        }

        println!("Icrc token canisters created");

        let bft_bridge = Self::init_bft_bridge_contract(&ctx).await;

        Ok(Self {
            ctx,
            tokens,
            contracts_deployer,
            bft_bridge,
            users: Default::default(),
        })
    }

    async fn init_bft_bridge_contract(ctx: &Ctx) -> H160 {
        println!("Initializing BFTBridge contract on base EVM");

        let erc20_bridge_client = ctx.erc20_bridge_client(ctx.admin_name());
        let bridge_canister_address = erc20_bridge_client
            .get_bridge_canister_evm_address()
            .await
            .unwrap()
            .unwrap();
        let base_wrapped_token_deployer = H160::default(); // We should not deploy wrapped tokens on base evm.

        let base_evm_client = ctx.external_evm_client(ctx.admin_name());
        let addr = ctx
            .initialize_bft_bridge_on_evm(
                &base_evm_client,
                bridge_canister_address,
                None,
                base_wrapped_token_deployer,
            )
            .await
            .unwrap();

        erc20_bridge_client
            .set_base_bft_bridge_contract(&addr)
            .await
            .unwrap();

        println!("BFTBridge contract initialized on base EVM");

        addr
    }
}

impl<Ctx: TestContext + Send + Sync> BaseTokens for Erc20BaseTokens<Ctx> {
    type TokenId = H160;
    type UserId = H160;

    fn ctx(&self) -> &(impl TestContext + Send + Sync) {
        &self.ctx
    }

    fn ids(&self) -> &[Self::TokenId] {
        &self.tokens
    }

    fn user_id256(&self, user_id: Self::UserId) -> Id256 {
        Id256::from_evm_address(&user_id, EXTERNAL_EVM_CHAIN_ID as _)
    }

    fn token_id256(&self, token_id: Self::TokenId) -> Id256 {
        Id256::from_evm_address(&token_id, EXTERNAL_EVM_CHAIN_ID as _)
    }

    fn next_memo(&self) -> [u8; 32] {
        let mut memo = [0u8; 32];
        let memo_value = MEMO_COUNTER.fetch_add(1, Ordering::Relaxed);
        memo[0..4].copy_from_slice(&memo_value.to_be_bytes());
        memo
    }

    async fn bridge_canister_evm_address(&self) -> Result<H160> {
        let client = self.ctx.icrc_bridge_client(self.ctx.admin_name());
        let address = client.get_bridge_canister_evm_address().await??;
        Ok(address)
    }

    async fn new_user(&self) -> Result<Self::UserId> {
        let wallet = OwnedWallet::new(&mut rand::thread_rng());
        let address = wallet.address();
        self.users.write().await.insert(address.into(), wallet);
        Ok(address.into())
    }

    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: U256) -> Result<()> {
        let token_address = self.tokens[token_idx].clone();

        let input = TestWTM::transferCall {
            to: to.clone().into(),
            value: amount.into(),
        }
        .abi_encode();

        let evm_client = self.ctx.external_evm_client(self.ctx.admin_name());
        let receipt = self
            .ctx
            .call_contract_on_evm(
                &evm_client,
                &self.contracts_deployer,
                &token_address,
                input,
                0,
            )
            .await?
            .1;
        assert_eq!(receipt.status, Some(U64::one()));

        Ok(())
    }

    async fn balance_of(&self, token_idx: usize, user: &Self::UserId) -> Result<U256> {
        let token_address = self.tokens[token_idx].clone();
        let evm_client = self.ctx.external_evm_client(self.ctx.admin_name());

        let balance = self
            .ctx
            .check_erc20_balance_on_evm(
                &evm_client,
                &token_address,
                &self.contracts_deployer,
                Some(user),
            )
            .await?;
        Ok(balance.into())
    }

    async fn deposit(
        &self,
        to_wallet: &Wallet<'_, SigningKey>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<U256> {
        let token_address = self.tokens[info.base_token_idx].clone();
        let evm_client = self.ctx.external_evm_client(self.ctx.admin_name());
        let sender_wallet = self.users.read().await.get(&info.from).cloned().unwrap();
        let to_token_id = self.token_id256(info.wrapped_token.clone());
        let recipient_id = self.user_id256(to_wallet.address().into());
        let memo = self.next_memo();

        self.ctx
            .burn_base_erc_20_tokens(
                &evm_client,
                &sender_wallet,
                &token_address,
                &to_token_id.0,
                recipient_id,
                &self.bft_bridge,
                info.amount.0.as_u128(),
                Some(memo),
            )
            .await?;

        Ok(info.amount.clone())
    }

    async fn set_bft_bridge_contract_address(&self, bft_bridge: &H160) -> Result<()> {
        self.ctx
            .erc20_bridge_client(self.ctx.admin_name())
            .set_bft_bridge_contract(bft_bridge)
            .await?;

        Ok(())
    }
}

/// Run stress test with the given TestContext implementation.
pub async fn stress_test_erc20_bridge_with_ctx<T>(
    ctx: T,
    base_tokens_number: usize,
    config: StressTestConfig,
) where
    T: TestContext + Send + Sync,
{
    let base_tokens = Erc20BaseTokens::init(ctx, base_tokens_number)
        .await
        .unwrap();
    let stress_test_stats = StressTestState::run(base_tokens, config).await.unwrap();

    dbg!(&stress_test_stats);

    assert_eq!(stress_test_stats.failed_deposits, 0);
    assert_eq!(stress_test_stats.failed_withdrawals, 0);
}
