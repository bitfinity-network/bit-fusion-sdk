use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use alloy_sol_types::SolCall;
use bridge_client::BridgeCanisterClient;
use bridge_did::id256::Id256;
use bridge_utils::BFTBridge;
use did::{TransactionReceipt, H160, H256, U256, U64};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_exports::ic_cdk::println;
use tokio::sync::RwLock;

use super::{BaseTokens, BurnInfo, StressTestConfig, StressTestState};
use crate::context::TestContext;
use crate::utils::error::Result;
use crate::utils::{TestWTM, CHAIN_ID};

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
        let tx_hash = external_evm_client
            .admin_mint_native_tokens(deployer_address.into(), u128::MAX.into())
            .await
            .unwrap()
            .unwrap()
            .0;

        let mint_tx_receipt = ctx
            .wait_transaction_receipt_on_evm(&external_evm_client, &tx_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(mint_tx_receipt.status, Some(U64::one()));

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

        // wait to allow bridge canister to query evm params from external EVM.
        ctx.advance_by_times(Duration::from_millis(500), 10).await;

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

    async fn wait_tx_success(&self, tx_hash: &H256) -> Result<TransactionReceipt> {
        let evm_client = self.ctx.external_evm_client(self.ctx.admin_name());
        let mut retries = 0;
        let receipt = loop {
            if retries > 10 {
                return Err(crate::utils::error::TestError::Generic(
                    "failed to get tx receipt".into(),
                ));
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
            let Some(receipt) = evm_client
                .eth_get_transaction_receipt(tx_hash.clone())
                .await??
            else {
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
        Id256::from_evm_address(&user_id, CHAIN_ID as _)
    }

    fn token_id256(&self, token_id: Self::TokenId) -> Id256 {
        Id256::from_evm_address(&token_id, CHAIN_ID as _)
    }

    fn next_memo(&self) -> [u8; 32] {
        let mut memo = [0u8; 32];
        let memo_value = MEMO_COUNTER.fetch_add(1, Ordering::Relaxed);
        memo[0..4].copy_from_slice(&memo_value.to_be_bytes());
        memo
    }

    async fn bridge_canister_evm_address(&self) -> Result<H160> {
        let client = self.ctx.erc20_bridge_client(self.ctx.admin_name());
        let address = client.get_bridge_canister_evm_address().await??;
        Ok(address)
    }

    async fn new_user(&self) -> Result<Self::UserId> {
        let wallet = OwnedWallet::new(&mut rand::thread_rng());
        let address = wallet.address();

        let client = self.ctx.external_evm_client(self.ctx.admin_name());
        let tx_hash = client
            .admin_mint_native_tokens(address.into(), u128::MAX.into())
            .await
            .unwrap()
            .unwrap()
            .0;

        let mint_tx_receipt = self
            .ctx
            .wait_transaction_receipt_on_evm(&client, &tx_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(mint_tx_receipt.status, Some(U64::one()));

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

        println!("approving tokens for bridge");
        let input = TestWTM::approveCall {
            spender: self.bft_bridge.clone().into(),
            value: info.amount.clone().into(),
        }
        .abi_encode();

        let tx_hash = self
            .ctx
            .call_contract_without_waiting_on_evm(
                &evm_client,
                &sender_wallet,
                &token_address,
                input,
                0,
            )
            .await?;

        self.wait_tx_success(&tx_hash).await?;

        println!("burning tokens for bridge");
        let input = BFTBridge::burnCall {
            amount: info.amount.clone().into(),
            fromERC20: token_address.clone().into(),
            toTokenID: alloy_sol_types::private::FixedBytes::from_slice(&to_token_id.0),
            recipientID: recipient_id.0.into(),
            memo: memo.into(),
        }
        .abi_encode();

        let tx_hash = self
            .ctx
            .call_contract_without_waiting_on_evm(
                &evm_client,
                &sender_wallet,
                &self.bft_bridge,
                input,
                0,
            )
            .await?;

        self.wait_tx_success(&tx_hash).await?;

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
