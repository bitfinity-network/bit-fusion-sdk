use bridge_did::op_id::OperationId;
use did::H160;
use ic_canister_client::{CanisterClient, CanisterClientResult};
use rune_bridge::ops::RuneBridgeOp;

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
    ) -> CanisterClientResult<Vec<(OperationId, RuneBridgeOp)>> {
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
