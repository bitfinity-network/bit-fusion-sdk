#![allow(async_fn_in_trait)]

use bridge_did::{error::BftResult, op_id::OperationId};

use crate::bridge::Operation;

use super::RuntimeState;

pub mod sing_orders;

pub trait BridgeService {
    async fn push_operation(&mut self, id: OperationId) -> BftResult<()>;
    async fn run(&mut self) -> BftResult<()>;
}
