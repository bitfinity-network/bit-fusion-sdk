#![allow(async_fn_in_trait)]

#[cfg(feature = "dfx_tests")]
pub mod brc20;
pub mod erc20;
pub mod icrc;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use alloy_sol_types::SolCall;
use bridge_did::id256::Id256;
use bridge_did::operation_log::Memo;
use bridge_utils::BFTBridge;
use did::{H160, U256};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use futures::future;
use ic_exports::ic_cdk::println;

use crate::context::TestContext;
use crate::utils::error::Result;

pub struct StressTestConfig {
    pub users_number: usize,
    pub user_deposits_per_token: usize,
    pub operation_timeout: Duration,
    pub init_user_balance: U256,
    pub operation_amount: U256,
    pub wait_per_iteration: Duration,
}

pub trait BaseTokens {
    type TokenId: Clone + Send + Sync;
    type UserId: Clone + Send + Sync;

    fn ctx(&self) -> &(impl TestContext + Send + Sync);
    fn ids(&self) -> &[Self::TokenId];

    /// Returns the bridged user id as bytes
    fn user_id(&self, user_id: Self::UserId) -> Vec<u8>;
    fn token_id256(&self, token_id: Self::TokenId) -> Id256;

    async fn bridge_canister_evm_address(&self) -> Result<H160>;

    async fn new_user(&self, wrapped_wallet: &OwnedWallet) -> Result<Self::UserId>;
    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: U256) -> Result<()>;
    async fn balance_of(&self, token_idx: usize, user: &Self::UserId) -> Result<U256>;

    async fn deposit(
        &self,
        to_user: &User<Self::UserId>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<U256>;

    async fn new_user_with_balance(
        &self,
        wrapped_wallet: &OwnedWallet,
        token_idx: usize,
        balance: U256,
    ) -> Result<Self::UserId> {
        let user = self.new_user(wrapped_wallet).await?;
        self.mint(token_idx, &user, balance).await?;
        Ok(user)
    }

    /// A hook that is called before the withdraw operation is executed.
    ///
    /// Implementation is optional.
    ///
    /// Some protocols require it to fund the withdraw operation, such as brc20 bridge.
    async fn before_withdraw(
        &self,
        _token_idx: usize,
        _user_id: Self::UserId,
        _user_wallet: &OwnedWallet,
        _amount: U256,
    ) -> Result<()> {
        Ok(())
    }

    /// Get the BFTBridge contract address, if already set in the inner context.
    ///
    /// Implementation is optional.
    async fn get_bft_bridge_contract_address(&self) -> Option<H160> {
        None
    }
    async fn set_bft_bridge_contract_address(&self, bft_bridge: &H160) -> Result<()>;

    async fn create_wrapped_token(
        &self,
        admin_wallet: &OwnedWallet,
        bft_bridge: &H160,
        token_id: Id256,
    ) -> Result<H160>;

    async fn is_operation_complete(&self, address: H160, memo: Memo) -> Result<bool>;
}

pub struct BurnInfo<UserId> {
    pub bridge: H160,
    pub base_token_idx: usize,
    pub wrapped_token: H160,
    pub from: UserId,
    pub amount: U256,
    pub memo: Memo,
}

pub type OwnedWallet = Wallet<'static, SigningKey>;

pub struct User<BaseId> {
    pub wallet: OwnedWallet,
    pub base_id: BaseId,
    pub nonce: AtomicU64,
}

impl<B> User<B> {
    pub fn next_nonce(&self) -> u64 {
        self.nonce.fetch_add(1, Ordering::Relaxed)
    }

    pub fn address(&self) -> H160 {
        self.wallet.address().into()
    }
}

pub struct StressTestState<'a, B: BaseTokens> {
    base_tokens: &'a B,
    users: Vec<User<B::UserId>>,
    wrapped_tokens: Vec<H160>,
    bft_bridge: H160,
    config: StressTestConfig,
    memo_counter: AtomicU64,
}

impl<'a, B: BaseTokens> StressTestState<'a, B> {
    pub async fn run(base_tokens: &'a B, config: StressTestConfig) -> Result<StressTestStats> {
        let admin_wallet = base_tokens.ctx().new_wallet(u64::MAX as _).await?;

        let wrapped_token_deployer = base_tokens
            .ctx()
            .initialize_wrapped_token_deployer_contract(&admin_wallet)
            .await
            .unwrap();

        let expected_fee_charge_address =
            ethers_core::utils::get_contract_address(admin_wallet.address(), 3);

        println!("Initializing BftBridge contract");
        let bridge_canister_address = base_tokens.bridge_canister_evm_address().await?;

        base_tokens
            .ctx()
            .evm_client(base_tokens.ctx().admin_name())
            .admin_mint_native_tokens(bridge_canister_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        base_tokens
            .ctx()
            .advance_by_times(Duration::from_secs(1), 2)
            .await;

        let existing_bft_bridge = base_tokens.get_bft_bridge_contract_address().await;

        let (bft_bridge, fee_charge_address) = match existing_bft_bridge {
            Some(bft_bridge) => (bft_bridge, None),
            None => {
                let bft_bridge = base_tokens
                    .ctx()
                    .initialize_bft_bridge_with_minter(
                        &admin_wallet,
                        bridge_canister_address,
                        Some(expected_fee_charge_address.into()),
                        wrapped_token_deployer,
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

                (bft_bridge, Some(fee_charge_address))
            }
        };

        println!("Creating wrapped tokens");
        let mut wrapped_tokens = Vec::with_capacity(base_tokens.ids().len());
        for base_id in base_tokens.ids() {
            let token_id256 = base_tokens.token_id256(base_id.clone());
            let wrapped_address = base_tokens
                .create_wrapped_token(&admin_wallet, &bft_bridge, token_id256)
                .await?;
            wrapped_tokens.push(wrapped_address);
        }

        // Create users and give them balance on each base token.
        println!("Initializing base token users with their balances");
        let mut users = Vec::with_capacity(config.users_number);
        for _ in 0..config.users_number {
            // Create a wallet for the user.
            let wallet = base_tokens.ctx().new_wallet(u64::MAX as _).await?;
            let user_id = base_tokens.new_user(&wallet).await?;

            for token_idx in 0..base_tokens.ids().len() {
                // Create a user with base token balance.
                base_tokens
                    .mint(token_idx, &user_id, config.init_user_balance.clone())
                    .await?;
            }

            // Deposit native token to charge fee.
            let evm_client = base_tokens.ctx().evm_client(base_tokens.ctx().admin_name());

            if let Some(fee_charge_address) = fee_charge_address.as_ref() {
                // TODO: must be changed, because ID256 shouldn't be used for this purpose!!!
                // TODO: see <https://infinityswap.atlassian.net/browse/EPROD-1062>
                let user_id = base_tokens.user_id(user_id.clone());
                let user_id_32: [u8; 32] = user_id.try_into().unwrap();
                let user_id256 = Id256(user_id_32);
                base_tokens
                    .ctx()
                    .native_token_deposit(
                        &evm_client,
                        fee_charge_address.clone(),
                        &wallet,
                        &[user_id256],
                        10_u128.pow(15),
                    )
                    .await?;
            } else {
                evm_client
                    .admin_mint_native_tokens(wallet.address().into(), u64::MAX.into())
                    .await
                    .unwrap()
                    .unwrap();
            }

            let user = User {
                wallet,
                base_id: user_id.clone(),
                nonce: AtomicU64::new(if fee_charge_address.is_some() { 1 } else { 0 }),
            };

            users.push(user);
        }

        let state = Self {
            base_tokens,
            wrapped_tokens,
            users,
            bft_bridge,
            config,
            memo_counter: Default::default(),
        };

        state.run_operations().await
    }

    fn next_memo(&self) -> [u8; 32] {
        let mut memo = [0u8; 32];
        let memo_value = self.memo_counter.fetch_add(1, Ordering::Relaxed);
        let memo_bytes = memo_value.to_be_bytes();
        memo[0..memo_bytes.len()].copy_from_slice(&memo_bytes);
        memo
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
        let mut roundtrip_futures = Vec::new();
        for token_idx in 0..self.wrapped_tokens.len() {
            for user_idx in 0..self.users.len() {
                let roundtrip = Box::pin(self.run_roundtrips(
                    token_idx,
                    user_idx,
                    self.config.user_deposits_per_token,
                ));
                roundtrip_futures.push(roundtrip);
            }
        }

        let roundtrips_finished = AtomicBool::default();
        let time_progression_future = async {
            while !roundtrips_finished.load(Ordering::Relaxed) {
                self.base_tokens
                    .ctx()
                    .advance_time(Duration::from_millis(200))
                    .await;
            }
        };

        // Run all roundtrips concurrently.
        let roundtrips_future = async {
            let roundtrips_result = future::join_all(roundtrip_futures).await;
            roundtrips_finished.store(true, Ordering::Relaxed);
            roundtrips_result
        };

        let (roundtrips_info, _) = tokio::join!(roundtrips_future, time_progression_future);

        let finish_bridge_canister_native_balance = self
            .base_tokens
            .ctx()
            .evm_client(self.base_tokens.ctx().admin_name())
            .eth_get_balance(
                self.base_tokens.bridge_canister_evm_address().await?,
                did::BlockNumber::Latest,
            )
            .await??;

        let successful_roundtrips = roundtrips_info.iter().sum();
        let expected_roundtrips = self.config.users_number
            * self.config.user_deposits_per_token
            * self.base_tokens.ids().len();
        let failed_roundtrips = expected_roundtrips - successful_roundtrips;

        Ok(StressTestStats {
            successful_roundtrips,
            failed_roundtrips,
            init_bridge_canister_native_balance,
            finish_bridge_canister_native_balance,
        })
    }

    async fn run_roundtrips(&self, token_idx: usize, user_idx: usize, repeat: usize) -> usize {
        let mut successes = 0;
        for _ in 0..repeat {
            let success = self.run_roundtrip(token_idx, user_idx).await;
            if success {
                successes += 1;
            }
        }
        successes
    }

    async fn run_roundtrip(&self, token_idx: usize, user_idx: usize) -> bool {
        let deposit_memo = match self.token_deposit(token_idx, user_idx).await {
            Ok(memo) => memo,
            Err(e) => {
                println!("failed to deposit tokens: {e}");
                return false;
            }
        };

        println!("waiting for user {user_idx} deposit in token {token_idx}");
        let user_address = self.users[user_idx].address();
        if !self
            .wait_operation_complete(user_address, deposit_memo)
            .await
        {
            println!("deposit of user {user_idx} for token {token_idx} timeout");
            return false;
        }

        println!("starting withdrawal for user {user_idx} in token {token_idx}");
        let withdraw_amount = self.config.operation_amount.0 / U256::from(2u64).0;
        let withdraw_memo = match self
            .withdraw(token_idx, user_idx, withdraw_amount.into())
            .await
        {
            Ok(memo) => memo,
            Err(e) => {
                println!("failed to withdraw tokens: {e}");
                return false;
            }
        };

        println!("waiting for user {user_idx} withdraw from token {token_idx}");
        let user_address = self.users[user_idx].address();
        if !self
            .wait_operation_complete(user_address, withdraw_memo)
            .await
        {
            println!("withdraw of user {user_idx} for token {token_idx} timeout");
            return false;
        }

        true
    }

    async fn wait_operation_complete(&self, address: H160, memo: Memo) -> bool {
        let start = Instant::now();
        while start.elapsed() < self.config.operation_timeout {
            tokio::time::sleep(self.config.wait_per_iteration).await;

            let complete = match self
                .base_tokens
                .is_operation_complete(address.clone(), memo)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    println!(
                        "failed to check operation completeness: {e}; elapsed: {}s/{}s",
                        start.elapsed().as_secs(),
                        self.config.operation_timeout.as_secs()
                    );
                    false
                }
            };
            if complete {
                return true;
            }
        }

        false
    }

    async fn token_deposit(&self, token_idx: usize, user_idx: usize) -> Result<Memo> {
        println!("Trying to deposit token#{token_idx} for user#{user_idx}");
        let user = &self.users[user_idx];
        let memo = self.next_memo();
        let burn_info = BurnInfo {
            bridge: self.bft_bridge.clone(),
            base_token_idx: token_idx,
            wrapped_token: self.wrapped_tokens[token_idx].clone(),
            from: user.base_id.clone(),
            amount: self.config.operation_amount.clone(),
            memo,
        };
        self.base_tokens.deposit(user, &burn_info).await?;

        Ok(memo)
    }

    async fn withdraw(&self, token_idx: usize, user_idx: usize, amount: U256) -> Result<Memo> {
        let token_id = self.base_tokens.ids()[token_idx].clone();
        let base_token_id: Id256 = self.base_tokens.token_id256(token_id);

        let user_id = self
            .base_tokens
            .user_id(self.users[user_idx].base_id.clone());

        self.base_tokens
            .before_withdraw(
                token_idx,
                self.users[user_idx].base_id.clone().clone(),
                &self.users[user_idx].wallet,
                amount.clone(),
            )
            .await?;

        let memo = self.next_memo();
        let input = BFTBridge::burnCall {
            amount: amount.into(),
            fromERC20: self.wrapped_tokens[token_idx].clone().into(),
            toTokenID: alloy_sol_types::private::FixedBytes::from_slice(&base_token_id.0),
            recipientID: user_id.into(),
            memo: memo.into(),
        }
        .abi_encode();

        let nonce = self.users[user_idx].next_nonce();
        self.base_tokens
            .ctx()
            .call_contract_without_waiting(
                &self.users[user_idx].wallet,
                &self.bft_bridge,
                input,
                0,
                Some(nonce),
            )
            .await?;

        Ok(memo)
    }
}

#[derive(Debug)]
pub struct StressTestStats {
    pub successful_roundtrips: usize,
    pub failed_roundtrips: usize,
    pub init_bridge_canister_native_balance: U256,
    pub finish_bridge_canister_native_balance: U256,
}
