use bridge_utils::operation_store::MinterOperationId;
use did::H160;
use ic_canister_client::{CanisterClient, CanisterClientResult};
use icrc2_minter::operation::OperationState;

use crate::context::bridge_client::BridgeCanisterClient;

pub struct Icrc2BridgeClient<C> {
    client: C,
}

impl<C: CanisterClient> Icrc2BridgeClient<C> {
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

impl<C: CanisterClient> BridgeCanisterClient<C> for Icrc2BridgeClient<C> {
    fn client(&self) -> &C {
        &self.client
    }
}
