use std::sync::Arc;
use std::time::Duration;

use candid::utils::ArgumentEncoder;
use candid::{encode_args, CandidType, Decode, Nat, Principal};
use eth_signer::ic_sign::SigningKeyId;
use ic_base_types::{CanisterId, PrincipalId};
use ic_canister_client::{CanisterClient, CanisterClientResult};
use ic_exports::ic_kit::mock_principals::{alice, bob};
use ic_exports::icrc_types::icrc1::account::Account;
use ic_management_canister_types::CanisterSettingsArgsBuilder;
use ic_state_machine_tests::StateMachine;
use serde::de::DeserializeOwned;

use crate::context::{TestCanisters, TestContext};
use crate::utils::error::TestError;

mod btc;
mod rune;

pub struct StateMachineContext {
    env: Arc<StateMachine>,
    canisters: TestCanisters,
}

impl StateMachineContext {
    pub fn new(env: StateMachine) -> Self {
        Self {
            env: Arc::new(env),
            canisters: TestCanisters::default(),
        }
    }
}

#[async_trait::async_trait]
impl<'a> TestContext for &'a StateMachineContext {
    type Client = StateMachineClient;

    fn canisters(&self) -> TestCanisters {
        self.canisters.clone()
    }

    fn client(&self, canister: Principal, caller_name: &str) -> Self::Client {
        let caller = self.principal_by_caller_name(caller_name);

        StateMachineClient {
            canister,
            caller,
            env: self.env.clone(),
        }
    }

    fn principal_by_caller_name(&self, caller: &str) -> Principal {
        match &caller.to_lowercase()[..] {
            "alice" => alice(),
            "bob" => bob(),
            "admin" => self.admin(),
            _ => Principal::anonymous(),
        }
    }

    fn admin(&self) -> Principal {
        bob()
    }

    fn admin_name(&self) -> &str {
        "admin"
    }

    fn sign_key(&self) -> SigningKeyId {
        todo!()
    }

    async fn advance_time(&self, time: Duration) {
        let env = self.env.clone();
        tokio::task::spawn_blocking(move || env.advance_time(time))
            .await
            .unwrap();
        self.env.tick();
    }

    async fn create_canister(&self) -> crate::utils::error::Result<Principal> {
        let env = self.env.clone();
        let args = CanisterSettingsArgsBuilder::new()
            .with_controller(self.admin().into())
            .build();
        Ok(tokio::task::spawn_blocking(move || {
            env.create_canister_with_cycles(None, 1_000_000_000_000_000u128.into(), Some(args))
                .into()
        })
        .await
        .unwrap())
    }

    async fn create_canister_with_id(
        &self,
        _id: Principal,
    ) -> crate::utils::error::Result<Principal> {
        todo!()
    }

    async fn install_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> crate::utils::error::Result<()> {
        let env = self.env.clone();
        let data = encode_args(args).unwrap();

        tokio::task::spawn_blocking(move || {
            env.install_existing_canister(
                CanisterId::try_from(PrincipalId(canister)).unwrap(),
                wasm,
                data,
            )
            .map_err(|err| TestError::Generic(format!("{err:?}")))
        })
        .await
        .unwrap()
    }

    async fn reinstall_canister(
        &self,
        _canister: Principal,
        _wasm: Vec<u8>,
        _args: impl ArgumentEncoder + Send,
    ) -> crate::utils::error::Result<()> {
        todo!()
    }

    async fn upgrade_canister(
        &self,
        _canister: Principal,
        _wasm: Vec<u8>,
        _args: impl ArgumentEncoder + Send,
    ) -> crate::utils::error::Result<()> {
        todo!()
    }

    fn icrc_token_initial_balances(&self) -> Vec<(Account, Nat)> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct StateMachineClient {
    canister: Principal,
    caller: Principal,
    env: Arc<StateMachine>,
}

#[async_trait::async_trait]
impl CanisterClient for StateMachineClient {
    async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        let env = self.env.clone();
        let data = encode_args(args).expect("Failed to encode data");
        let sender = self.caller.into();
        let canister = self.canister;
        let method = method.to_string();

        let result = tokio::task::spawn_blocking(move || {
            env.execute_ingress_as(
                sender,
                CanisterId::try_from(PrincipalId(canister)).unwrap(),
                method,
                data,
            )
            .expect("request failed")
        })
        .await
        .unwrap();

        Ok(Decode!(&result.bytes(), R).expect("failed to decode result"))
    }

    async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        let env = self.env.clone();
        let data = encode_args(args).expect("Failed to encode data");
        let sender = self.caller.into();
        let canister = self.canister;
        let method = method.to_string();

        let result = tokio::task::spawn_blocking(move || {
            env.execute_ingress_as(
                sender,
                CanisterId::try_from(PrincipalId(canister)).unwrap(),
                method,
                data,
            )
            .expect("request failed")
        })
        .await
        .unwrap();

        Ok(Decode!(&result.bytes(), R).expect("failed to decode result"))
    }
}
