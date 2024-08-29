#![allow(async_fn_in_trait)]

pub mod icrc;

use std::time::Duration;

use alloy_sol_types::SolCall;
use bridge_did::id256::Id256;
use bridge_utils::BFTBridge;
use did::{H160, U256};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use futures::future;

use crate::context::TestContext;
use crate::utils::error::Result;

pub struct StressTestConfig {
    pub users_number: usize,
    pub user_deposits_per_token: usize,
    pub init_user_balance: U256,
    pub operation_amount: U256,
}

pub trait BaseTokens {
    type TokenId: Into<Id256> + Clone + Send + Sync;
    type UserId: Clone + Send + Sync;

    fn ctx(&self) -> &(impl TestContext + Send + Sync);
    fn ids(&self) -> &[Self::TokenId];
    fn user_id256(&self, user_id: Self::UserId) -> Id256;

    async fn bridge_canister_evm_address(&self) -> Result<H160>;

    async fn new_user(&self) -> Result<Self::UserId>;
    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: U256) -> Result<()>;
    async fn balance_of(&self, token_idx: usize, user: &Self::UserId) -> Result<U256>;

    async fn deposit(
        &self,
        to_wallet: &Wallet<'_, SigningKey>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<U256>;

    async fn new_user_with_balance(&self, token_idx: usize, balance: U256) -> Result<Self::UserId> {
        let user = self.new_user().await?;
        self.mint(token_idx, &user, balance).await?;
        Ok(user)
    }

    async fn set_bft_bridge_contract_address(&self, bft_bridge: &H160) -> Result<()>;
}

pub struct BurnInfo<UserId> {
    pub bridge: H160,
    pub base_token_idx: usize,
    pub wrapped_token: H160,
    pub from: UserId,
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
            ethers_core::utils::get_contract_address(admin_wallet.address(), 2);

        println!("Initializing BftBridge contract");
        let bridge_canister_address = base_tokens.bridge_canister_evm_address().await?;

        base_tokens
            .ctx()
            .evm_client(base_tokens.ctx().admin_name())
            .mint_native_tokens(bridge_canister_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        base_tokens
            .ctx()
            .advance_by_times(Duration::from_secs(1), 2)
            .await;

        let bft_bridge = base_tokens
            .ctx()
            .initialize_bft_bridge_with_minter(
                &admin_wallet,
                bridge_canister_address,
                Some(expected_fee_charge_address.into()),
                true,
            )
            .await?;

        println!("Initializing fee charge contract");
        let fee_charge_address = base_tokens
            .ctx()
            .initialize_fee_charge_contract(&admin_wallet, &[bft_bridge.clone()])
            .await
            .unwrap();
        assert_eq!(expected_fee_charge_address, fee_charge_address.0);

        base_tokens
            .set_bft_bridge_contract_address(&bft_bridge)
            .await?;

        println!("Creating wrapped tokens");
        let mut wrapped_tokens = Vec::with_capacity(base_tokens.ids().len());
        for base_id in base_tokens.ids() {
            let wrapped_address = base_tokens
                .ctx()
                .create_wrapped_token(&admin_wallet, &bft_bridge, base_id.clone().into())
                .await?;
            wrapped_tokens.push(wrapped_address);
        }

        // Create users and give them balance on each base token.
        println!("Initializing base token users with their balances");
        let mut users = Vec::with_capacity(config.users_number);
        for _ in 0..config.users_number {
            let user_id = base_tokens.new_user().await?;

            for token_idx in 0..base_tokens.ids().len() {
                // Create a user with base token balance.
                base_tokens
                    .mint(token_idx, &user_id, config.init_user_balance.clone())
                    .await?;
            }

            // Create a wallet for the user.
            let user_wallet = base_tokens.ctx().new_wallet(u64::MAX as _).await?;
            // Deposit native token to charge fee.
            let evm_client = base_tokens.ctx().evm_client(base_tokens.ctx().admin_name());
            let user_id256 = base_tokens.user_id256(user_id.clone());
            base_tokens
                .ctx()
                .native_token_deposit(
                    &evm_client,
                    fee_charge_address.clone(),
                    &user_wallet,
                    &[user_id256],
                    10_u128.pow(15),
                )
                .await?;

            users.push((user_wallet, user_id.clone()));
        }

        let state = Self {
            base_tokens,
            wrapped_tokens,
            users,
            bft_bridge,
            config,
        };

        state.run_operations().await
    }

    async fn run_operations(self) -> Result<StressTestStats> {
        println!("Starting deposit/withdraw operations");
        let init_bridge_canister_native_balance = self
            .base_tokens
            .ctx()
            .evm_client(self.base_tokens.ctx().admin_name())
            .eth_get_balance(
                self.base_tokens.bridge_canister_evm_address().await?,
                did::BlockNumber::Latest,
            )
            .await??;

        // Prepare deposits and withdrawals
        let mut deposits_futures = Vec::new();
        let mut withdrawals_futures = Vec::new();
        for token_idx in 0..self.wrapped_tokens.len() {
            for user_idx in 0..self.users.len() {
                let deposit = Box::pin(self.token_deposit(
                    token_idx,
                    user_idx,
                    self.config.user_deposits_per_token,
                ));
                deposits_futures.push(deposit);

                let withdrawal = Box::pin(self.withdraw_on_positive_balance(token_idx, user_idx));
                withdrawals_futures.push(withdrawal);
            }
        }

        let time_progression_future = async {
            for _ in 0..1000 {
                self.base_tokens
                    .ctx()
                    .advance_time(Duration::from_millis(200))
                    .await;
            }
        };

        // Run all the operations concurrently.
        let deposit_future = future::join_all(deposits_futures);
        let withdrawal_future = future::join_all(withdrawals_futures);

        let (deposit_results, withdrawal_results, _) =
            tokio::join!(deposit_future, withdrawal_future, time_progression_future);

        let mut successful_deposits = 0;
        let mut failed_deposits = 0;
        for result in deposit_results {
            match result {
                Ok(_) => successful_deposits += 1,
                Err(e) => {
                    failed_deposits += 1;
                    eprintln!("deposit failed: {e}");
                }
            }
        }

        let mut successful_withdrawals = 0;
        let mut failed_withdrawals = 0;
        for result in withdrawal_results {
            match result {
                Ok(_) => successful_withdrawals += 1,
                Err(e) => {
                    failed_withdrawals += 1;
                    eprintln!("withdrawal failed: {e}");
                }
            }
        }

        let finish_bridge_canister_native_balance = self
            .base_tokens
            .ctx()
            .evm_client(self.base_tokens.ctx().admin_name())
            .eth_get_balance(
                self.base_tokens.bridge_canister_evm_address().await?,
                did::BlockNumber::Latest,
            )
            .await??;

        Ok(StressTestStats {
            successful_deposits,
            failed_deposits,
            successful_withdrawals,
            failed_withdrawals,
            init_bridge_canister_native_balance,
            finish_bridge_canister_native_balance,
        })
    }

    async fn token_deposit(&self, token_idx: usize, user_idx: usize, repeat: usize) -> Result<()> {
        for _ in 0..repeat {
            println!("Trying to deposit token#{token_idx} for user#{user_idx}");
            let user = &self.users[user_idx];
            let burn_info = BurnInfo {
                bridge: self.bft_bridge.clone(),
                base_token_idx: token_idx,
                wrapped_token: self.wrapped_tokens[token_idx].clone(),
                from: user.1.clone(),
                amount: self.config.operation_amount.clone(),
            };
            self.base_tokens.deposit(&user.0, &burn_info).await?;
        }

        Ok(())
    }

    async fn withdraw_on_positive_balance(&self, token_idx: usize, user_idx: usize) -> Result<()> {
        println!("Trying to withdraw token#{token_idx} for user#{user_idx}");
        const WAIT_FOR_BALANCE_TIMEOUT: Duration = Duration::from_secs(60);
        let balance_future = self.wait_for_wrapped_balance(token_idx, user_idx);
        let balance_result = tokio::time::timeout(WAIT_FOR_BALANCE_TIMEOUT, balance_future).await;

        if let Ok(Ok(balance)) = balance_result {
            self.withdraw(token_idx, user_idx, balance).await?;
        }

        Ok(())
    }

    async fn wait_for_wrapped_balance(&self, token_idx: usize, user_idx: usize) -> Result<U256> {
        loop {
            let balance = self
                .base_tokens
                .ctx()
                .check_erc20_balance(
                    &self.wrapped_tokens[token_idx],
                    &self.users[user_idx].0,
                    None,
                )
                .await?;

            if balance > 0 {
                return Ok(balance.into());
            }
        }
    }

    async fn withdraw(&self, token_idx: usize, user_idx: usize, amount: U256) -> Result<()> {
        let base_token_id: Id256 = self.base_tokens.ids()[token_idx].clone().into();
        let user_id256 = self.base_tokens.user_id256(self.users[user_idx].1.clone());

        let input = BFTBridge::burnCall {
            amount: amount.into(),
            fromERC20: self.wrapped_tokens[token_idx].clone().into(),
            toTokenID: alloy_sol_types::private::FixedBytes::from_slice(&base_token_id.0),
            recipientID: user_id256.0.into(),
        }
        .abi_encode();

        self.base_tokens
            .ctx()
            .call_contract_without_waiting(&self.users[user_idx].0, &self.bft_bridge, input, 0)
            .await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct StressTestStats {
    pub successful_deposits: usize,
    pub failed_deposits: usize,
    pub successful_withdrawals: usize,
    pub failed_withdrawals: usize,
    pub init_bridge_canister_native_balance: U256,
    pub finish_bridge_canister_native_balance: U256,
}
