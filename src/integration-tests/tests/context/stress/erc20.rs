use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use alloy_sol_types::SolCall;
use bridge_client::BridgeCanisterClient;
use bridge_did::id256::Id256;
use bridge_did::operation_log::Memo;
use bridge_did::operations::Erc20OpStage;
use bridge_utils::BTFBridge;
use did::{TransactionReceipt, H160, H256, U256, U64};
use eth_signer::{Signer, Wallet};
use ic_exports::ic_cdk::println;
use tokio::sync::RwLock;

use super::{BaseTokens, BurnInfo, OwnedWallet, StressTestConfig, StressTestState, User};
use crate::context::TestContext;
use crate::utils::error::{Result, TestError};
use crate::utils::{GanacheEvm, TestEvm as _, TestWTM, CHAIN_ID};

pub struct Erc20BaseTokens<Ctx> {
    ctx: Ctx,
    tokens: Vec<H160>,
    contracts_deployer: OwnedWallet,
    btf_bridge: H160,
    nonces: RwLock<HashMap<H160, AtomicU64>>,
}

impl<Ctx: TestContext<GanacheEvm> + Send + Sync> Erc20BaseTokens<Ctx> {
    async fn init(ctx: Ctx, base_tokens_number: usize) -> Result<Self> {
        let base_evm_client = ctx.base_evm();

        // Create contract deployer wallet.
        let contracts_deployer = Wallet::new(&mut rand::thread_rng());
        let deployer_address = contracts_deployer.address();
        base_evm_client
            .mint_native_tokens(deployer_address.into(), u128::MAX.into())
            .await
            .expect("mint_native_tokens failed");

        let mut tokens = Vec::with_capacity(base_tokens_number);
        for _ in 0..base_tokens_number {
            let icrc_principal = ctx
                .deploy_test_wtm_token_on_evm(
                    &base_evm_client,
                    &contracts_deployer,
                    u128::MAX.into(),
                )
                .await
                .unwrap();
            tokens.push(icrc_principal);
        }

        println!("Base Erc20 token contracts created");

        // wait to allow bridge canister to query evm params from external EVM.
        ctx.advance_by_times(Duration::from_millis(500), 10).await;

        let btf_bridge = Self::init_btf_bridge_contract(&ctx).await;

        // Mint tokens for bridge canister
        let bridge_client = ctx.erc20_bridge_client(ctx.admin_name());
        let bridge_address = bridge_client
            .get_bridge_canister_base_evm_address()
            .await
            .unwrap()
            .unwrap();
        base_evm_client
            .mint_native_tokens(bridge_address, u128::MAX.into())
            .await
            .expect("mint_native_tokens failed");

        Ok(Self {
            ctx,
            tokens,
            contracts_deployer,
            btf_bridge,
            nonces: Default::default(),
        })
    }

    async fn init_btf_bridge_contract(ctx: &Ctx) -> H160 {
        println!("Initializing BTFBridge contract on base EVM");

        let erc20_bridge_client = ctx.erc20_bridge_client(ctx.admin_name());
        let bridge_canister_address = erc20_bridge_client
            .get_bridge_canister_evm_address()
            .await
            .unwrap()
            .unwrap();
        let base_wrapped_token_deployer = H160::default(); // We should not deploy wrapped tokens on base evm.

        let base_evm_client = ctx.base_evm();
        let addr = ctx
            .initialize_btf_bridge_on_evm(
                &base_evm_client,
                bridge_canister_address,
                None,
                base_wrapped_token_deployer,
                false,
            )
            .await
            .unwrap();

        erc20_bridge_client
            .set_base_btf_bridge_contract(&addr)
            .await
            .unwrap();

        println!("BTFBridge contract initialized on base EVM");

        addr
    }

    async fn wait_tx_success(&self, tx_hash: &H256) -> Result<TransactionReceipt> {
        let evm_client = self.ctx.wrapped_evm();
        let mut retries = 0;
        let receipt = loop {
            if retries > 100 {
                return Err(crate::utils::error::TestError::Generic(
                    "failed to get tx receipt".into(),
                ));
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
            let Some(receipt) = evm_client.get_transaction_receipt(tx_hash).await? else {
                retries += 1;
                continue;
            };

            break receipt;
        };

        if receipt.status != Some(U64::one()) {
            let output = receipt.output.unwrap_or_default();
            let output_str = String::from_utf8_lossy(&output);
            println!("tx failed with ouptput: {output_str}");
            return Err(crate::utils::error::TestError::Generic("tx failed".into()));
        }

        Ok(receipt)
    }

    async fn next_nonce(&self, address: &H160) -> u64 {
        self.nonces
            .read()
            .await
            .get(address)
            .unwrap()
            .fetch_add(1, Ordering::Relaxed)
    }
}

impl<Ctx: TestContext<GanacheEvm> + Send + Sync> BaseTokens for Erc20BaseTokens<Ctx> {
    type TokenId = H160;
    type UserId = H160;
    type EVM = GanacheEvm;

    fn ctx(&self) -> &(impl TestContext<GanacheEvm> + Send + Sync) {
        &self.ctx
    }

    fn ids(&self) -> &[Self::TokenId] {
        &self.tokens
    }

    async fn user_id(&self, user_id: Self::UserId) -> Vec<u8> {
        Id256::from_evm_address(&user_id, CHAIN_ID as _).0.to_vec()
    }

    fn token_id256(&self, token_id: Self::TokenId) -> Id256 {
        Id256::from_evm_address(&token_id, CHAIN_ID as _)
    }

    async fn bridge_canister_evm_address(&self) -> Result<H160> {
        let client = self.ctx.erc20_bridge_client(self.ctx.admin_name());
        let address = client.get_bridge_canister_evm_address().await??;
        Ok(address)
    }

    async fn new_user(&self, wrapped_wallet: &OwnedWallet) -> Result<Self::UserId> {
        let address = wrapped_wallet.address();
        let client = self.ctx.wrapped_evm();
        client
            .mint_native_tokens(address.into(), u128::MAX.into())
            .await
            .expect("mint_native_tokens failed");

        self.nonces
            .write()
            .await
            .insert(address.into(), AtomicU64::default());

        Ok(address.into())
    }

    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: U256) -> Result<()> {
        let token_address = self.tokens[token_idx].clone();

        let input = TestWTM::transferCall {
            to: to.clone().into(),
            value: amount.into(),
        }
        .abi_encode();

        let evm_client = self.ctx.wrapped_evm();
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
        let evm_client = self.ctx.wrapped_evm();

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
        to_user: &User<Self::UserId>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<U256> {
        let user_wallet = to_user.wallet.clone();
        let user_address = user_wallet.address();
        let token_address = self.tokens[info.base_token_idx].clone();
        let evm_client = self.ctx.wrapped_evm();
        let nonce = self.next_nonce(&user_address.into()).await;
        let to_token_id = self.token_id256(info.wrapped_token.clone());
        let recipient_id = self.user_id(user_address.into()).await;
        let memo = info.memo;

        println!("approving tokens for bridge");
        let input = TestWTM::approveCall {
            spender: self.btf_bridge.clone().into(),
            value: info.amount.clone().into(),
        }
        .abi_encode();

        let tx_hash = self
            .ctx
            .call_contract_without_waiting_on_evm(
                &evm_client,
                &user_wallet,
                &token_address,
                input,
                0,
                Some(nonce),
            )
            .await?;

        self.wait_tx_success(&tx_hash).await?;

        println!("burning tokens for bridge");
        let input = BTFBridge::burnCall {
            amount: info.amount.clone().into(),
            fromERC20: token_address.clone().into(),
            toTokenID: alloy_sol_types::private::FixedBytes::from_slice(&to_token_id.0),
            recipientID: recipient_id.into(),
            memo: memo.into(),
        }
        .abi_encode();

        let nonce = self.next_nonce(&user_address.into()).await;
        let tx_hash = self
            .ctx
            .call_contract_without_waiting_on_evm(
                &evm_client,
                &user_wallet,
                &self.btf_bridge,
                input,
                0,
                Some(nonce),
            )
            .await?;

        self.wait_tx_success(&tx_hash).await?;

        Ok(info.amount.clone())
    }

    async fn set_btf_bridge_contract_address(&self, btf_bridge: &H160) -> Result<()> {
        self.ctx
            .erc20_bridge_client(self.ctx.admin_name())
            .set_btf_bridge_contract(btf_bridge)
            .await?;

        Ok(())
    }

    async fn is_operation_complete(&self, address: H160, memo: Memo) -> Result<bool> {
        let Some(operation) = self
            .ctx
            .erc20_bridge_client(self.ctx.admin_name())
            .get_operation_by_memo_and_user(memo, &address)
            .await?
        else {
            return Err(TestError::Generic("operation not found".into()));
        };

        let is_complete = matches!(operation.1.stage, Erc20OpStage::TokenMintConfirmed(_));
        Ok(is_complete)
    }

    async fn create_wrapped_token(
        &self,
        admin_wallet: &OwnedWallet,
        bft_bridge: &H160,
        token_id: Id256,
    ) -> Result<H160> {
        self.ctx
            .create_wrapped_token(admin_wallet, bft_bridge, token_id)
            .await
    }
}

/// Run stress test with the given TestContext implementation.
pub async fn stress_test_erc20_bridge_with_ctx<T>(
    ctx: T,
    base_tokens_number: usize,
    config: StressTestConfig,
) where
    T: TestContext<GanacheEvm> + Send + Sync,
{
    let base_tokens = Erc20BaseTokens::init(ctx, base_tokens_number)
        .await
        .unwrap();
    let stress_test_stats = StressTestState::run(&base_tokens, config).await.unwrap();

    dbg!(&stress_test_stats);

    assert_eq!(stress_test_stats.failed_roundtrips, 0);
    assert!(
        stress_test_stats.init_bridge_canister_native_balance
            <= stress_test_stats.finish_bridge_canister_native_balance
    );
}
