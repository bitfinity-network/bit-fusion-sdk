use bridge_did::op_id::OperationId;
use bridge_did::operation_log::OperationLog;
use bridge_did::operations::RuneBridgeOp;
use candid::{CandidType, Deserialize};
use did::H160;
use ic_canister_client::{CanisterClient, CanisterClientResult};

use crate::bridge_client::BridgeCanisterClient;

pub struct RuneBridgeClient<C> {
    client: C,
}

/// TODO: DELETE AND MOVE INTO DID
#[derive(Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum GetAddressError {
    Key(String),
}

impl<C: CanisterClient> RuneBridgeClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }


    pub async fn admin_configure_ecdsa(&self) -> CanisterClientResult<()> {
        self.client
            .update("admin_configure_ecdsa", ())
            .await
    }

    pub async fn get_deposit_address(
        &self,
        eth_address: &H160,
    ) -> CanisterClientResult<Result<String, GetAddressError>> {
        self.client
            .update("get_deposit_address", (eth_address,))
            .await
    }

    pub async fn get_operations_list(
        &self,
        wallet_address: &H160,
    ) -> CanisterClientResult<Vec<(OperationId, RuneBridgeOp)>> {
        self.client
            .update("get_operations_list", (wallet_address,))
            .await
    }

    pub async fn get_operation_log(
        &self,
        operation_id: OperationId,
    ) -> CanisterClientResult<Option<OperationLog<RuneBridgeOp>>> {
        self.client
            .query("get_operation_log", (operation_id,))
            .await
    }
}

impl<C: CanisterClient> BridgeCanisterClient<C> for RuneBridgeClient<C> {
    fn client(&self) -> &C {
        &self.client
    }
}
