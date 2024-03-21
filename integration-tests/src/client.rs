use candid::Principal;
use did::build::BuildData;
use ic_canister_client::{CanisterClient, CanisterClientResult};

use crate::error::InscriberCallResult;

/// An Inscriber canister client.
#[derive(Debug, Clone)]
pub struct InscriberCanisterClient<C>
where
    C: CanisterClient,
{
    /// The canister client.
    client: C,
}

impl<C: CanisterClient> InscriberCanisterClient<C> {
    /// Create a new canister client.
    ///
    /// # Arguments
    /// * `client` - The canister client.
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// Get the owner of the canister
    pub async fn get_owner(&self) -> CanisterClientResult<Principal> {
        self.client.query("get_owner", ()).await
    }

    /// Set the owner of the canister
    pub async fn set_owner(
        &self,
        principal: Principal,
    ) -> CanisterClientResult<InscriberCallResult<()>> {
        self.client.update("set_owner", (principal,)).await
    }

    /// Returns the build data of the canister.
    pub async fn get_canister_build_data(&self) -> CanisterClientResult<BuildData> {
        self.client.query("get_canister_build_data", ()).await
    }
}
