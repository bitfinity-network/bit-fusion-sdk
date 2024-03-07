use crate::context::{TestCanisters, TestContext};
use crate::utils::error::TestError;
use candid::utils::ArgumentEncoder;
use candid::{encode_args, CandidType, Decode, Nat, Principal};
use ic_base_types::{CanisterId, PrincipalId};
use ic_canister_client::{CanisterClient, CanisterClientResult};
use ic_exports::ic_kit::mock_principals::{alice, bob};
use ic_state_machine_tests::StateMachine;
use icrc_ledger_types::icrc1::account::Account;
use serde::de::DeserializeOwned;
use std::time::Duration;

mod btc;

pub struct StateMachineContext {
    env: StateMachine,
    canisters: TestCanisters,
}

impl StateMachineContext {
    pub fn new(env: StateMachine) -> Self {
        Self {
            env,
            canisters: TestCanisters::default(),
        }
    }
}

#[async_trait::async_trait]
impl<'a> TestContext for &'a StateMachineContext {
    type Client = StateMachineClient<'a>;

    fn canisters(&self) -> TestCanisters {
        self.canisters.clone()
    }

    fn client(&self, canister: Principal, caller_name: &str) -> Self::Client {
        let caller = match &caller_name.to_lowercase()[..] {
            "alice" => alice(),
            "bob" => bob(),
            "admin" => self.admin(),
            _ => Principal::anonymous(),
        };

        StateMachineClient {
            canister,
            caller,
            env: &self.env,
        }
    }

    fn admin(&self) -> Principal {
        bob()
    }

    fn admin_name(&self) -> &str {
        "admin"
    }

    async fn advance_time(&self, time: Duration) {
        self.env.advance_time(time)
    }

    async fn create_canister(&self) -> crate::utils::error::Result<Principal> {
        Ok(self.env.create_canister(None).into())
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
        let data = encode_args(args).unwrap();
        self.env
            .install_existing_canister(
                CanisterId::try_from(PrincipalId(canister)).unwrap(),
                wasm,
                data,
            )
            .map_err(|err| TestError::Generic(format!("{err:?}")))
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
pub struct StateMachineClient<'a> {
    canister: Principal,
    caller: Principal,
    env: &'a StateMachine,
}

#[async_trait::async_trait]
impl<'a> CanisterClient for StateMachineClient<'a> {
    async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        let data = encode_args(args).expect("Failed to encode data");
        let result = self
            .env
            .execute_ingress_as(
                self.caller.into(),
                CanisterId::try_from(PrincipalId(self.canister)).unwrap(),
                method,
                data,
            )
            .expect("request failed");
        Ok(Decode!(&result.bytes(), R).expect("failed to decode result"))
    }

    async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        let data = encode_args(args).expect("Failed to encode data");
        let result = self
            .env
            .execute_ingress(
                CanisterId::try_from(PrincipalId(self.canister)).unwrap(),
                method,
                data,
            )
            .expect("request failed");
        Ok(Decode!(&result.bytes(), R).expect("failed to decode result"))
    }
}
