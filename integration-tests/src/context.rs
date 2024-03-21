use std::collections::HashMap;

use candid::utils::ArgumentEncoder;
use candid::Principal;
use ic_canister_client::CanisterClient;

use crate::client::InscriberCanisterClient;
use crate::error::TestResult;
use crate::utils;

#[async_trait::async_trait]
pub trait TestContext {
    type Client: CanisterClient + Send + Sync;

    /// Returns principal for canster in the context.
    fn canisters(&self) -> TestCanisters;

    /// Returns client for the canister.
    fn client(&self, canister: Principal, caller: &str) -> Self::Client;

    /// Principal to use for canister's initialization.
    fn admin(&self) -> Principal;

    /// Principal to use for canister's initialization.
    fn admin_name(&self) -> &str;

    /// Returns client for the Inscriber canister.
    fn inscriber_client(&self, caller: &str) -> InscriberCanisterClient<Self::Client> {
        InscriberCanisterClient::new(self.client(self.canisters().inscriber(), caller))
    }

    /// Creates an empty canister with cycles on its balance.
    async fn create_canister(&self) -> TestResult<Principal>;

    /// Installs the `wasm` code to the `canister` with the given init `args`.
    async fn install_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> TestResult<()>;

    /// Reinstalls the canister.
    async fn reinstall_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> TestResult<()>;

    /// Upgrades the canister.
    async fn upgrade_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> TestResult<()>;
}

#[derive(Debug, Clone, Default)]
pub struct TestCanisters(HashMap<CanisterType, Principal>);

impl TestCanisters {
    pub fn inscriber(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Inscriber)
            .expect("inscriber canister should be initialized (see `TestContext::new()`)")
    }

    pub fn btc_bridge(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::BtcBridge)
            .expect("bridge canister should be initialized (see `TestContext::new()`)")
    }

    pub fn set(&mut self, canister_type: CanisterType, principal: Principal) {
        self.0.insert(canister_type, principal);
    }

    pub fn get_or_anonymous(&self, canister_type: CanisterType) -> Principal {
        self.0
            .get(&canister_type)
            .copied()
            .unwrap_or_else(Principal::anonymous)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanisterType {
    Inscriber,
    BtcBridge,
}

impl CanisterType {
    pub async fn default_canister_wasm(&self) -> Vec<u8> {
        match self {
            CanisterType::Inscriber => utils::get_inscriber_canister_bytecode().await,
            CanisterType::BtcBridge => utils::get_btc_bridge_canister_bytecode().await,
        }
    }
}
