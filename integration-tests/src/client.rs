use candid::utils::ArgumentEncoder;
use candid::{CandidType, Principal};
use ic_canister::virtual_canister_call;
use ic_canister_client::{CanisterClient, CanisterClientError, CanisterClientResult};
use serde::de::DeserializeOwned;

/// An Inscriber canister client.
#[derive(Debug, Clone)]
pub struct InscriberCanisterClient {
    /// The canister id of the Inscriber canister
    pub canister_id: Principal,
}

impl InscriberCanisterClient {
    pub fn new(canister_id: Principal) -> Self {
        Self { canister_id }
    }

    async fn call<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send,
        R: DeserializeOwned + CandidType,
    {
        virtual_canister_call!(self.canister_id, method, args, R)
            .await
            .map_err(CanisterClientError::CanisterError)
    }
}

#[async_trait::async_trait]
impl CanisterClient for InscriberCanisterClient {
    async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        self.call(method, args).await
    }

    async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        self.call(method, args).await
    }
}
