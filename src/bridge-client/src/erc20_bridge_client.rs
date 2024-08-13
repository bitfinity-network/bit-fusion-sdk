use bridge_did::op_id::OperationId;
use bridge_utils::common::Pagination;
use did::H160;
use erc20_bridge::ops::Erc20BridgeOp;
use ic_canister_client::{CanisterClient, CanisterClientResult};

use crate::bridge_client::BridgeCanisterClient;

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
        pagination: Option<Pagination>,
    ) -> CanisterClientResult<Vec<(OperationId, Erc20BridgeOp)>> {
        self.client
            .update("get_operations_list", (wallet_address, pagination))
            .await
    }

    pub async fn set_base_bft_bridge_contract(&self, address: &H160) -> CanisterClientResult<()> {
        self.client
            .update("set_base_bft_bridge_contract", (address,))
            .await
    }
}

impl<C: CanisterClient> BridgeCanisterClient<C> for Erc20BridgeClient<C> {
    fn client(&self) -> &C {
        &self.client
    }
}
