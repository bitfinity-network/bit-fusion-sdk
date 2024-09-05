use bridge_did::op_id::OperationId;
use bridge_did::operation_log::OperationLog;
use bridge_did::operations::IcrcBridgeOp;
use bridge_utils::common::Pagination;
use did::H160;
use ic_canister_client::{CanisterClient, CanisterClientResult};

use crate::bridge_client::BridgeCanisterClient;

pub struct Icrc2BridgeClient<C> {
    client: C,
}

impl<C: CanisterClient> Icrc2BridgeClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// Returns list of operations for the given parameters.
    pub async fn get_operations_list(
        &self,
        wallet_address: &H160,
        pagination: Option<Pagination>,
    ) -> CanisterClientResult<Vec<(OperationId, IcrcBridgeOp)>> {
        self.client
            .query("get_operations_list", (wallet_address, pagination))
            .await
    }

    pub async fn get_operation_log(
        &self,
        operation_id: OperationId,
    ) -> CanisterClientResult<Option<OperationLog<IcrcBridgeOp>>> {
        self.client
            .query("get_operation_log", (operation_id,))
            .await
    }
}

impl<C: CanisterClient> BridgeCanisterClient<C> for Icrc2BridgeClient<C> {
    fn client(&self) -> &C {
        &self.client
    }
}
