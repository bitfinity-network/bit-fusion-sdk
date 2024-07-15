use bridge_utils::operation_store::MinterOperationId;
use did::H160;
use ic_canister_client::{CanisterClient, CanisterClientResult};
use rune_bridge::operation::OperationState;

use crate::bridge_client::BridgeCanisterClient;

pub struct RuneBridgeClient<C> {
    client: C,
}

impl<C: CanisterClient> RuneBridgeClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub async fn get_operations_list(
        &self,
        wallet_address: &H160,
    ) -> CanisterClientResult<Vec<(MinterOperationId, OperationState)>> {
        self.client
            .update("get_operations_list", (wallet_address,))
            .await
    }
}

impl<C: CanisterClient> BridgeCanisterClient<C> for RuneBridgeClient<C> {
    fn client(&self) -> &C {
        &self.client
    }
}
