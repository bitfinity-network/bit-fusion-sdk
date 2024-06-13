pub mod erc20_minter;
pub mod icrc2_minter;
mod token;

use std::fmt;
use std::time::Duration;

use candid::utils::ArgumentEncoder;
use candid::{Nat, Principal};
use did::{TransactionReceipt, H160, H256};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::PocketIcClient;
use ic_exports::ic_kit::mock_principals::{alice, bob, john};
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::pocket_ic::nio::PocketIcAsync;

use crate::context::{CanisterType, TestCanisters, TestContext, ICRC1_INITIAL_BALANCE};
use crate::utils::error::Result;
use crate::utils::EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS;

const ADMIN: &str = "admin";
const JOHN: &str = "john";
const ALICE: &str = "alice";

pub struct PocketIcTestContext {
    pub client: PocketIcAsync,
    pub canisters: TestCanisters,
}

impl PocketIcTestContext {
    pub async fn new(canisters_set: &[CanisterType]) -> Self {
        let client = PocketIcAsync::init().await;
        let mut ctx = PocketIcTestContext {
            client,
            canisters: TestCanisters::default(),
        };

        for canister_type in canisters_set {
            let principal = ctx
                .create_canister()
                .await
                .expect("canister should be created");

            ctx.canisters.set(*canister_type, principal);
        }

        for canister_type in canisters_set {
            ctx.install_default_canister(*canister_type).await;
        }

        ctx
    }

    pub fn admin() -> Principal {
        bob()
    }

    pub fn principal_of(name: &str) -> Principal {
        match name {
            ADMIN => Self::admin(),
            JOHN => john(),
            ALICE => alice(),
            _ => panic!("unexpected caller"),
        }
    }
}

#[async_trait::async_trait]
impl TestContext for PocketIcTestContext {
    type Client = PocketIcClient;

    fn canisters(&self) -> TestCanisters {
        self.canisters.clone()
    }

    async fn advance_time(&self, time: Duration) {
        self.client.advance_time(time).await;
        self.client.tick().await;
    }

    fn client(&self, canister: Principal, caller: &str) -> Self::Client {
        let caller_principal = Self::principal_of(caller);
        PocketIcClient::from_client(self.client.clone(), canister, caller_principal)
    }

    fn principal_by_caller_name(&self, caller: &str) -> Principal {
        Self::principal_of(caller)
    }

    fn admin(&self) -> Principal {
        Self::admin()
    }

    fn admin_name(&self) -> &str {
        ADMIN
    }

    async fn create_canister(&self) -> Result<Principal> {
        let principal = self.client.create_canister(Some(self.admin())).await;
        self.client.add_cycles(principal, u128::MAX).await;
        Ok(principal)
    }

    async fn create_canister_with_id(&self, id: Principal) -> Result<Principal> {
        let sync_client = ic_exports::pocket_ic::PocketIc::new();
        sync_client
            .create_canister_with_id(Some(self.admin()), None, id)
            .expect("failed to create canister");
        self.client.add_cycles(id, u128::MAX).await;
        Ok(id)
    }

    async fn install_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()> {
        let args = candid::encode_args(args).unwrap();
        self.client
            .install_canister(canister, wasm, args, Some(self.admin()))
            .await;
        Ok(())
    }

    async fn reinstall_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()> {
        let args = candid::encode_args(args).unwrap();
        self.client
            .reinstall_canister(canister, wasm, args, Some(self.admin()))
            .await
            .unwrap();
        Ok(())
    }

    async fn upgrade_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()> {
        let args = candid::encode_args(args).unwrap();
        self.client
            .upgrade_canister(canister, wasm, args, Some(self.admin()))
            .await
            .unwrap();

        Ok(())
    }

    /// Waits for transaction receipt.
    async fn wait_transaction_receipt_on_evm(
        &self,
        evm_client: &EvmCanisterClient<Self::Client>,
        hash: &H256,
    ) -> Result<Option<TransactionReceipt>> {
        for _ in 0..50 {
            let time = EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS.mul_f64(1.1);
            self.advance_time(time).await;
            let result = evm_client.eth_get_transaction_receipt(hash.clone()).await?;

            if result.is_err() {
                println!("failed to get tx receipt: {result:?}")
            }

            let receipt = result?;
            if receipt.is_some() {
                return Ok(receipt);
            }
        }

        Ok(None)
    }

    fn icrc_token_initial_balances(&self) -> Vec<(Account, Nat)> {
        vec![
            (Account::from(bob()), Nat::from(ICRC1_INITIAL_BALANCE)),
            (Account::from(john()), Nat::from(ICRC1_INITIAL_BALANCE)),
            (Account::from(alice()), Nat::from(ICRC1_INITIAL_BALANCE)),
        ]
    }
}

impl fmt::Debug for PocketIcTestContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PocketIcTestContext")
            .field("env", &"PocketIcTestContext tests client")
            .field("canisters", &self.canisters)
            .finish()
    }
}

/// Initialize test environment with:
/// - john wallet with native tokens,
/// - operation points for john,
/// - bridge contract
async fn init_bridge() -> (PocketIcTestContext, Wallet<'static, SigningKey>, H160, H160) {
    let ctx = PocketIcTestContext::new(&CanisterType::ICRC2_MINTER_TEST_SET).await;
    let john_wallet = ctx.new_wallet(u128::MAX).await.unwrap();

    let fee_charge_deployer = ctx.new_wallet(u128::MAX).await.unwrap();
    let expected_fee_charge_address =
        ethers_core::utils::get_contract_address(fee_charge_deployer.address(), 0);

    let bft_bridge = ctx
        .initialize_bft_bridge(ADMIN, expected_fee_charge_address.into())
        .await
        .unwrap();

    let fee_charge_address = ctx
        .initialize_fee_charge_contract(&fee_charge_deployer, &[bft_bridge.clone()])
        .await
        .unwrap();
    assert_eq!(expected_fee_charge_address, fee_charge_address.0);

    (ctx, john_wallet, bft_bridge, fee_charge_address)
}
