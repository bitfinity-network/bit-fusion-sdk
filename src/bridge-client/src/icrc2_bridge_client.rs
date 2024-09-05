use bridge_did::operation_log::OperationLog;
use bridge_did::{op_id::OperationId, operation_log::Memo};
use bridge_utils::common::Pagination;
use did::H160;
use ic_canister_client::{CanisterClient, CanisterClientResult};
use icrc2_bridge::ops::IcrcBridgeOp;

use crate::bridge_client::BridgeCanisterClient;

pub struct Icrc2BridgeClient<C> {
    client: C,
}

impl<C: CanisterClient> Icrc2BridgeClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// Returns list of operatinst for the given parameters.
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

    pub async fn get_operation_by_memo_and_user(
        &self,
        memo: Memo,
        user_id: H160,
    ) -> CanisterClientResult<Option<(OperationId, IcrcBridgeOp)>> {
        self.client
            .query("get_operation_by_memo_and_user", (memo, user_id))
            .await
    }

    pub async fn get_operations_by_memo(
        &self,
        memo: Memo,
    ) -> CanisterClientResult<Vec<(H160, OperationId, IcrcBridgeOp)>> {
        self.client.query("get_operations_by_memo", (memo,)).await
    }
}

impl<C: CanisterClient> BridgeCanisterClient<C> for Icrc2BridgeClient<C> {
    fn client(&self) -> &C {
        &self.client
    }
}
