use std::collections::HashMap;

use bridge_did::id256::Id256;
use bridge_utils::bft_events::{BurntEventData, MintedEventData};
use did::{H160, U256};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;

use crate::context::TestContext;
use crate::dfx_tests::ADMIN;
use crate::utils::error::Result;

pub struct StressTestConfig {
    pub users_number: usize,
    pub operations_per_user: usize,
    pub init_user_balance: U256,
    pub operation_amount: U256,
}

pub trait BaseTokens {
    type TokenId: Into<Id256> + TryFrom<Id256> + Clone + Send + Sync;
    type UserId: Into<Id256> + TryFrom<Id256> + Clone + Send + Sync;

    fn ctx(&self) -> &(impl TestContext + Send + Sync);
    fn ids(&self) -> &[Self::TokenId];

    async fn bridge_canister_evm_address(&self) -> Result<H160>;

    async fn new_user(&self) -> Result<Self::UserId>;
    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: U256) -> Result<()>;

    async fn deposit(&self, info: &BurnInfo<Self::UserId>) -> Result<U256>;
    async fn on_withdraw(&self, token_idx: usize, memo: u64) -> Result<U256>;

    async fn new_user_with_balance(&self, token_idx: usize, balance: U256) -> Result<Self::UserId> {
        let user = self.new_user().await?;
        self.mint(token_idx, &user, balance).await?;
        Ok(user)
    }
}

pub struct BurnInfo<UserId> {
    pub bridge: H160,
    pub base_token_idx: usize,
    pub from: UserId,
    pub to: H160,
    pub amount: U256,
}

pub struct StressTestState<B: BaseTokens> {
    base_tokens: B,
    users: Vec<(Wallet<'static, SigningKey>, B::UserId)>,
    wrapped_tokens: Vec<H160>,
    bft_bridge: H160,
    config: StressTestConfig,
}

impl<B: BaseTokens> StressTestState<B> {
    pub async fn run(base_tokens: B, config: StressTestConfig) -> Result<StressTestStats> {
        let admin_wallet = base_tokens.ctx().new_wallet(u64::MAX as _).await?;

        let expected_fee_charge_address =
            ethers_core::utils::get_contract_address(admin_wallet.address(), 0);

        let bridge_canister_address = base_tokens.bridge_canister_evm_address().await?;
        let bft_bridge = base_tokens
            .ctx()
            .initialize_bft_bridge_with_minter(
                &admin_wallet,
                bridge_canister_address,
                Some(expected_fee_charge_address.into()),
                true,
            )
            .await?;

        let fee_charge_address = base_tokens
            .ctx()
            .initialize_fee_charge_contract(&admin_wallet, &[bft_bridge.clone()])
            .await
            .unwrap();
        assert_eq!(expected_fee_charge_address, fee_charge_address.0);

        let mut wrapped_tokens = Vec::with_capacity(base_tokens.ids().len());
        for base_id in base_tokens.ids() {
            let wrapped_address = base_tokens
                .ctx()
                .create_wrapped_token(&admin_wallet, &bft_bridge, base_id.clone().into())
                .await?;
            wrapped_tokens.push(wrapped_address);
        }

        // Create users and give them balance on each base token.
        let mut users = Vec::with_capacity(config.users_number);
        for _ in 0..config.users_number {
            for token_idx in 0..base_tokens.ids().len() {
                // Create a user with base token balance.
                let user_id = base_tokens
                    .new_user_with_balance(token_idx, config.init_user_balance.clone())
                    .await?;

                // Create a wallet for the user.
                let user_wallet = base_tokens.ctx().new_wallet(u64::MAX as _).await?;

                // Deposit native token to charge fee.
                let evm_client = base_tokens.ctx().evm_client(ADMIN);
                base_tokens
                    .ctx()
                    .native_token_deposit(
                        &evm_client,
                        fee_charge_address.clone(),
                        &user_wallet,
                        &[user_id.clone().into()],
                        10_u128.pow(20),
                    )
                    .await?;
                users.push((user_wallet, user_id));
            }
        }

        let state = Self {
            base_tokens,
            wrapped_tokens,
            users,
            bft_bridge,
            config,
        };

        state.run_operations().await;

        todo!()
    }

    async fn run_operations(self) {
        let expected_deposits_number = self.config.operations_per_user * self.users.len();
    }
}

pub struct StressTestStats {
    pub expected_deposits_number: u64,
    pub deposits_number: u64,
    pub total_deposit_amount: U256,
    pub expected_withdraws_number: u64,
    pub withdraws_number: u64,
    pub total_withdraw_amount: U256,
    pub init_bridge_canister_native_balance: U256,
    pub finish_bridge_canister_native_balance: U256,
}
