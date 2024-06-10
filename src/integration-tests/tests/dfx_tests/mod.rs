use std::time::Duration;

use candid::utils::ArgumentEncoder;
use candid::{Nat, Principal};
use ic_canister_client::IcAgentClient;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_test_utils::{get_agent, Agent, Canister};
use ic_utils::interfaces::ManagementCanister;
use minter_contract_utils::evm_link::{EvmLink, RpcApi, RpcService};

use crate::context::{CanisterType, TestCanisters, TestContext};
use crate::utils::error::{Result, TestError};

mod ck_erc20;
mod erc20_minter;
mod runes;

const DFX_URL: &str = "http://127.0.0.1:4943";
pub const INIT_CANISTER_CYCLES: u128 = 900_000_000_000;

/// The name of the user with a thick wallet.
pub const ADMIN: &str = "max";
/// other identities available
pub const ALICE: &str = "alice";
pub const ALEX: &str = "alex";

/// The required setup for the dfx tests
pub struct DfxTestContext {
    canisters: TestCanisters,
    max: Agent,
    alice: Agent,
    alex: Agent,
}

impl DfxTestContext {
    pub async fn new(canisters_set: &[CanisterType]) -> Self {
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

        let mut ctx = Self {
            canisters: TestCanisters::default(),
            max,
            alice,
            alex,
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

    pub async fn stop_canister(&self, canister: Principal) -> Result<()> {
        let agent = self.agent_by_name(ADMIN);
        let management = Canister::new_management(&agent);
        management.stop_canister(&agent, canister).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl TestContext for DfxTestContext {
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
        EvmLink::EvmRpcCanister {
            canister_id: self.canisters().evm_rpc(),
            rpc_service: vec![RpcService::Custom(RpcApi {
                url: format!(
                    "http://127.0.0.1:8000/?canisterId={}",
                    self.canisters().external_evm()
                ),
                headers: None,
            })],
        }
    }

    /// Creates an empty canister with cycles on it's balance.
    async fn create_canister(&self) -> Result<Principal> {
        let wallet = Canister::new_wallet(&self.max, ADMIN).unwrap();
        let principal = wallet
            .create_canister(INIT_CANISTER_CYCLES as _, None)
            .await?;
        Ok(principal)
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
        todo!()
    }

    fn icrc_token_initial_balances(&self) -> Vec<(Account, Nat)> {
        todo!()
    }
}
