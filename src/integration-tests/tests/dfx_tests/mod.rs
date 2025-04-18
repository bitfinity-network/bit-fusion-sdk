use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge_did::evm_link::EvmLink;
use candid::utils::ArgumentEncoder;
use candid::{Nat, Principal};
use eth_signer::ic_sign::SigningKeyId;
use ic_canister_client::IcAgentClient;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_test_utils::{Agent, Canister, get_agent};
use ic_utils::interfaces::ManagementCanister;

use crate::context::{CanisterType, TestCanisters, TestCanistersConfig, TestContext};
use crate::utils::TestEvm;
use crate::utils::error::{Result, TestError};

mod brc20_bridge;
mod bridge_deployer;
mod rune_bridge;

const DFX_URL: &str = "http://127.0.0.1:4943";
pub const INIT_CANISTER_CYCLES: u64 = 90_000_000_000_000;

/// The name of the user with a thick wallet.
pub const ADMIN: &str = "max";
/// other identities available
pub const ALICE: &str = "alice";
pub const ALEX: &str = "alex";

/// The required setup for the dfx tests
pub struct DfxTestContext<EVM>
where
    EVM: TestEvm,
{
    alex: Agent,
    alice: Agent,
    base_evm: Arc<EVM>,
    canisters: TestCanisters,
    max: Agent,
    wrapped_evm: Arc<EVM>,
}

impl<EVM> DfxTestContext<EVM>
where
    EVM: TestEvm,
{
    pub async fn new(
        canisters_set: &[CanisterType],
        base_evm: Arc<EVM>,
        wrapped_evm: Arc<EVM>,
    ) -> Self {
        let url = Some(DFX_URL);
        let max = get_agent(ADMIN, url, Some(Duration::from_secs(180)))
            .await
            .unwrap();
        let alice = get_agent(ALICE, url, Some(Duration::from_secs(180)))
            .await
            .unwrap();
        let alex = get_agent(ALEX, url, Some(Duration::from_secs(180)))
            .await
            .unwrap();

        let test_canisters_config = TestCanistersConfig {
            evm: base_evm.evm(),
            external_evm: wrapped_evm.evm(),
            signature: base_evm.signature(),
            external_signature: wrapped_evm.signature(),
        };

        let mut ctx = Self {
            alex,
            alice,
            base_evm,
            canisters: TestCanisters::new(test_canisters_config),
            max,
            wrapped_evm,
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

    pub fn agent_by_name(&self, name: &str) -> Agent {
        match name {
            ADMIN => self.max.clone(),
            ALICE => self.alice.clone(),
            ALEX => self.alex.clone(),
            _ => panic!("Unknown agent: {name}"),
        }
    }
}

#[async_trait::async_trait]
impl<EVM> TestContext<EVM> for DfxTestContext<EVM>
where
    EVM: TestEvm,
{
    type Client = IcAgentClient;

    fn client(&self, canister: Principal, name: &str) -> Self::Client {
        let agent = self.agent_by_name(name);
        IcAgentClient::with_agent(canister, agent)
    }

    fn principal_by_caller_name(&self, caller: &str) -> Principal {
        self.agent_by_name(caller).get_principal().unwrap()
    }

    fn admin(&self) -> Principal {
        self.agent_by_name(ADMIN).get_principal().unwrap()
    }

    fn admin_name(&self) -> &str {
        ADMIN
    }

    fn canisters(&self) -> TestCanisters {
        self.canisters.clone()
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

    /// Creates an empty canister with cycles on it's balance.
    async fn create_canister(&self) -> Result<Principal> {
        let wallet = Canister::new_wallet(&self.max, ADMIN).unwrap();
        let principal = wallet.create_canister(INIT_CANISTER_CYCLES, None).await?;

        Ok(principal)
    }

    async fn stop_canister(&self, canister: Principal) -> Result<()> {
        let agent = self.agent_by_name(ADMIN);
        let management = Canister::new_management(&agent);
        management.stop_canister(&agent, canister).await?;
        Ok(())
    }

    /// Installs the `wasm` code to the `canister` with the given init `args`.
    async fn install_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()> {
        let mng = ManagementCanister::create(&self.max);
        mng.install(&canister, &wasm)
            .with_args(args)
            .call_and_wait()
            .await
            .map_err(|err| TestError::Generic(format!("Failed to install canister: {err:?}")))
    }

    async fn install_canister_with_sender(
        &self,
        _canister: Principal,
        _wasm: Vec<u8>,
        _args: impl ArgumentEncoder + Send,
        _sender: Principal,
    ) -> Result<()> {
        unimplemented!()
    }

    async fn reinstall_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()> {
        let agent = self.agent_by_name(ADMIN);
        ic_test_utils::reinstall_canister(&agent, canister, wasm.into(), args).await?;
        Ok(())
    }

    async fn upgrade_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()> {
        let agent = self.agent_by_name(ADMIN);
        let management = Canister::new_management(&agent);
        management
            .upgrade_code(&agent, canister, wasm.into(), args)
            .await?;
        Ok(())
    }

    async fn advance_time(&self, time: Duration) {
        tokio::time::sleep(time).await
    }

    async fn create_canister_with_id(&self, _id: Principal) -> Result<Principal> {
        unimplemented!()
    }

    async fn create_canister_with_id_and_controller(
        &self,
        _id: Principal,
        _owner: Principal,
    ) -> Result<Principal> {
        unimplemented!()
    }

    fn icrc_token_initial_balances(&self) -> Vec<(Account, Nat)> {
        unimplemented!()
    }

    fn sign_key(&self) -> SigningKeyId {
        SigningKeyId::Dfx
    }
}

/// Blocks until the predicate returns [`Ok`].
///
/// If the predicate does not return [`Ok`] within `max_wait`, the function panics.
/// Returns the value inside of the [`Ok`] variant of the predicate.
pub async fn block_until_succeeds<F, T, EVM>(
    predicate: F,
    ctx: &DfxTestContext<EVM>,
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
