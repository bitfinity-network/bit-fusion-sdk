pub mod btc_bridge;
pub mod erc20_bridge;
pub mod icrc2_bridge;
mod token;

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge_did::evm_link::EvmLink;
use candid::utils::ArgumentEncoder;
use candid::{Nat, Principal};
use eth_signer::ic_sign::SigningKeyId;
use ic_canister_client::PocketIcClient;
use ic_exports::ic_kit::mock_principals::{alice, bob, john};
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::pocket_ic::{PocketIc, PocketIcBuilder};

use crate::context::{CanisterType, TestCanisters, TestContext, ICRC1_INITIAL_BALANCE};
use crate::utils::error::Result;
use crate::utils::TestEvm;

pub const ADMIN: &str = "admin";
pub const JOHN: &str = "john";
pub const ALICE: &str = "alice";

#[derive(Clone)]
pub struct PocketIcTestContext<EVM>
where
    EVM: TestEvm,
{
    base_evm: Arc<EVM>,
    pub client: Arc<PocketIc>,
    pub canisters: TestCanisters,
    wrapped_evm: Arc<EVM>,
    live: bool,
}

impl<EVM> PocketIcTestContext<EVM>
where
    EVM: TestEvm,
{
    /// Creates a new test context with the given canisters.
    pub async fn new(
        canisters_set: &[CanisterType],
        base_evm: Arc<EVM>,
        wrapped_evm: Arc<EVM>,
    ) -> Self {
        if base_evm.live() || wrapped_evm.live() {
            Self::new_live(canisters_set, base_evm, wrapped_evm).await
        } else {
            Self::new_with(
                canisters_set,
                |builder| builder,
                |pic| Box::pin(async move { pic }),
                false,
                base_evm,
                wrapped_evm,
            )
            .await
        }
    }

    /// Creates a new test context with the given canisters and with the PocketIC instance in live mode.
    async fn new_live(
        canisters_set: &[CanisterType],
        base_evm: Arc<EVM>,
        wrapped_evm: Arc<EVM>,
    ) -> Self {
        Self::new_with(
            canisters_set,
            |builder| builder,
            |mut pic| {
                Box::pin(async move {
                    pic.set_time(std::time::SystemTime::now()).await;
                    pic.make_live(None).await;
                    pic
                })
            },
            true,
            base_evm,
            wrapped_evm,
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
        base_evm: Arc<EVM>,
        wrapped_evm: Arc<EVM>,
    ) -> Self
    where
        FB: FnOnce(PocketIcBuilder) -> PocketIcBuilder,
        FPIC: FnOnce(PocketIc) -> Pin<Box<dyn Future<Output = PocketIc> + Send + 'static>>,
    {
        let mut pocket_ic = with_build(ic_exports::pocket_ic::init_pocket_ic().await)
            .build_async()
            .await;

        pocket_ic = with_pocket_ic(pocket_ic).await;

        let client = Arc::new(pocket_ic);
        let mut ctx = PocketIcTestContext {
            base_evm,
            client,
            canisters: TestCanisters::default(),
            wrapped_evm,
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
impl<EVM> TestContext<EVM> for PocketIcTestContext<EVM>
where
    EVM: TestEvm,
{
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

    fn base_evm_link(&self) -> EvmLink {
        self.base_evm.link()
    }

    fn wrapped_evm_link(&self) -> EvmLink {
        self.wrapped_evm.link()
    }

    fn base_evm(&self) -> Arc<EVM> {
        self.base_evm.clone()
    }

    fn wrapped_evm(&self) -> Arc<EVM> {
        self.wrapped_evm.clone()
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

impl<EVM> fmt::Debug for PocketIcTestContext<EVM>
where
    EVM: TestEvm,
{
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
pub async fn block_until_succeeds<F, T, EVM>(
    predicate: F,
    ctx: &PocketIcTestContext<EVM>,
    max_wait: Duration,
) -> T
where
    F: Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<T>>>>,
    EVM: TestEvm,
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
