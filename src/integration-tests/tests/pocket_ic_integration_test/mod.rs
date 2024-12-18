pub mod brc20_bridge;
pub mod btc_bridge;
pub mod erc20_bridge;
pub mod icrc2_bridge;
pub mod rune_bridge;
mod token;

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use candid::utils::ArgumentEncoder;
use candid::{Nat, Principal};
use did::{TransactionReceipt, H256};
use eth_signer::ic_sign::SigningKeyId;
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::PocketIcClient;
use ic_exports::ic_kit::mock_principals::{alice, bob, john};
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::pocket_ic::{PocketIc, PocketIcBuilder};

use crate::context::{CanisterType, TestCanisters, TestContext, ICRC1_INITIAL_BALANCE};
use crate::utils::error::Result;
use crate::utils::EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS;

pub const ADMIN: &str = "admin";
pub const JOHN: &str = "john";
pub const ALICE: &str = "alice";

#[derive(Clone)]
pub struct PocketIcTestContext {
    pub client: Arc<PocketIc>,
    pub canisters: TestCanisters,
    live: bool,
}

impl PocketIcTestContext {
    /// Creates a new test context with the given canisters.
    pub async fn new(canisters_set: &[CanisterType]) -> Self {
        Self::new_with(
            canisters_set,
            |builder| builder,
            |pic| Box::pin(async move { pic }),
            false,
        )
        .await
    }

    pub async fn new_live(canisters_set: &[CanisterType]) -> Self {
        Self::new_with(
            canisters_set,
            |builder| builder,
            |pic| Box::pin(async move { pic }),
            false,
        )
        .await
    }

    /// Creates a new test context with the given canisters and custom build and pocket_ic.
    ///
    /// # Arguments
    ///
    /// * `canisters_set` - The set of canisters to create.
    /// * `with_build` - A closure that takes a `PocketIcBuilder` and returns a `PocketIcBuilder`.
    /// * `with_pocket_ic` - A closure that takes a `PocketIc` and returns a `Future` that resolves to a `PocketIc`.
    ///
    /// # Example
    ///
    /// ```
    /// use ic_test_utilities::pocket_ic::PocketIcTestContext;
    /// use ic_test_utilities::context::CanisterType;
    ///
    /// let canisters_set = vec![CanisterType::ICRC1];
    ///
    /// let ctx = PocketIcTestContext::new_with(
    ///    &canisters_set,
    ///    |builder| builder.with_ii_subnet().with_bitcoin_subnet(),
    ///    |mut pic| Box::pin(async move {
    ///        pic.make_live(None).await;
    ///        pic
    ///    }),
    /// ).await;
    /// ```
    pub async fn new_with<FB, FPIC>(
        canisters_set: &[CanisterType],
        with_build: FB,
        with_pocket_ic: FPIC,
        live: bool,
    ) -> Self
    where
        FB: FnOnce(PocketIcBuilder) -> PocketIcBuilder,
        FPIC: FnOnce(PocketIc) -> Pin<Box<dyn Future<Output = PocketIc> + Send + 'static>>,
    {
        let mut pocket_ic = ic_exports::pocket_ic::init_pocket_ic_with(with_build)
            .await
            .build_async()
            .await;

        pocket_ic = with_pocket_ic(pocket_ic).await;

        let client = Arc::new(pocket_ic);
        let mut ctx = PocketIcTestContext {
            client,
            canisters: TestCanisters::default(),
            live,
        };

        for canister_type in canisters_set {
            let principal = ctx
                .create_canister()
                .await
                .expect("canister should be created");
            println!(
                "Created canister {:?} with principal {}",
                canister_type, principal
            );

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
            _ => {
                let name = format!("user {name}");
                let bytes = name.as_bytes();
                Principal::from_slice(bytes)
            }
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
        if self.live {
            tokio::time::sleep(time).await;
        } else {
            self.client.advance_time(time).await;
            self.client.tick().await;
        }
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
        let principal = self
            .client
            .create_canister_with_settings(Some(self.admin()), None)
            .await;
        self.client.add_cycles(principal, u128::MAX).await;
        Ok(principal)
    }

    async fn create_canister_with_id(&self, id: Principal) -> Result<Principal> {
        self.create_canister_with_id_and_controller(id, self.admin())
            .await
    }

    async fn create_canister_with_id_and_controller(
        &self,
        id: Principal,
        owner: Principal,
    ) -> Result<Principal> {
        self.client
            .create_canister_with_id(Some(owner), None, id)
            .await
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
        self.install_canister_with_sender(canister, wasm, args, self.admin())
            .await
    }

    async fn install_canister_with_sender(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
        sender: Principal,
    ) -> Result<()> {
        let args = candid::encode_args(args).unwrap();
        self.client
            .install_canister(canister, wasm, args, Some(sender))
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
        for _ in 0..200 {
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

    fn sign_key(&self) -> SigningKeyId {
        SigningKeyId::PocketIc
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

/// Blocks until the predicate returns [`Ok`].
///
/// If the predicate does not return [`Ok`] within `max_wait`, the function panics.
/// Returns the value inside of the [`Ok`] variant of the predicate.
pub async fn block_until_succeeds<F, T>(
    predicate: F,
    ctx: &PocketIcTestContext,
    max_wait: Duration,
) -> T
where
    F: Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<T>>>>,
{
    let start = Instant::now();
    let mut err = anyhow::Error::msg("Predicate did not succeed within the given time");
    while start.elapsed() < max_wait {
        match predicate().await {
            Ok(res) => return res,
            Err(e) => err = e,
        }
        ctx.advance_time(Duration::from_millis(100)).await;
    }

    panic!(
        "Predicate did not succeed within {}s: {err}",
        max_wait.as_secs()
    );
}
