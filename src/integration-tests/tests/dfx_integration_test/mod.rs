mod inspect;

use std::time::Duration;

use ::evm_canister_client::ic_agent::Agent;
use candid::utils::ArgumentEncoder;
use candid::{Nat, Principal};
use ic_canister_client::IcAgentClient;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_test_utils::{get_agent, Canister};

use crate::context::{CanisterType, TestCanisters, TestContext};
use crate::utils::error::Result;

pub const INIT_CANISTER_CYCLES: u128 = 200_000_000_000;

/// The name of the user with a thick wallet.
pub const ADMIN: &str = "max";
/// other identities available
pub const ALICE: &str = "alice";
pub const ALEX: &str = "alex";

pub const INIT_ICRC1_BALANCE: u64 = 10_u64.pow(18);

/// The required setup for the dfx tests
pub struct DfxTestContext {
    canisters: TestCanisters,
    max: Agent,
    alice: Agent,
    alex: Agent,
}

impl DfxTestContext {
    pub async fn new(canisters_set: &[CanisterType]) -> Self {
        let max = get_agent(ADMIN, None, Some(Duration::from_secs(180)))
            .await
            .unwrap();
        let alice = get_agent(ALICE, None, Some(Duration::from_secs(180)))
            .await
            .unwrap();
        let alex = get_agent(ALEX, None, Some(Duration::from_secs(180)))
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

    pub async fn assert_inspect_message_failure<T: ArgumentEncoder>(
        &self,
        agent: &Agent,
        canister_id: Principal,
        method: &str,
        args: T,
    ) {
        println!("checking {method} for inspect message failure");
        let result = agent
            .update(&canister_id, method)
            .with_arg(candid::encode_args(args).unwrap())
            .call_and_wait()
            .await;
        if !Self::is_inspect_message_error(&result) {
            panic!("Expected inspect message error, got: {result:?}")
        }
    }

    pub async fn assert_inspect_message_success<T: ArgumentEncoder>(
        &self,
        agent: &Agent,
        canister_id: Principal,
        method: &str,
        args: T,
    ) {
        println!("checking {method} for inspect message success");
        let result = agent
            .update(&canister_id, method)
            .with_arg(candid::encode_args(args).unwrap())
            .call_and_wait()
            .await;
        if Self::is_inspect_message_error(&result) {
            panic!("Expected inspect message error, got: {result:?}")
        }
    }

    pub fn is_inspect_message_error<T, E: ToString>(result: &std::result::Result<T, E>) -> bool {
        matches!(result, Err(e)
        if e.to_string().contains("rejected by inspect check"))
    }
}

#[async_trait::async_trait]
impl TestContext for DfxTestContext {
    type Client = IcAgentClient;

    fn client(&self, canister: Principal, name: &str) -> Self::Client {
        let agent = self.agent_by_name(name);
        IcAgentClient::with_agent(canister, agent)
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
        let management = Canister::new_management(&self.max);
        management
            .install_code(&self.max, canister, wasm.into(), args)
            .await?;
        Ok(())
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

    fn icrc_token_initial_balances(&self) -> Vec<(Account, Nat)> {
        vec![(self.admin().into(), INIT_ICRC1_BALANCE.into())]
    }
}
