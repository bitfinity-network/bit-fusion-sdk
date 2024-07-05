use bridge_utils::operation_store::MinterOperationId;
use did::H160;
use erc20_minter::operation::OperationPayload;
use ic_canister_client::{CanisterClient, CanisterClientResult};

use crate::context::bridge_client::BridgeCanisterClient;

pub struct Erc20BridgeClient<C> {
    client: C,
}

impl<C: CanisterClient> Erc20BridgeClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub async fn get_operations_list(
        &self,
        wallet_address: &H160,
    ) -> CanisterClientResult<Vec<(MinterOperationId, OperationPayload)>> {
        self.client
            .update("get_operations_list", (wallet_address,))
            .await
    }
}

impl<C: CanisterClient> BridgeCanisterClient<C> for Erc20BridgeClient<C> {
    fn client(&self) -> &C {
        &self.client
    }
}
