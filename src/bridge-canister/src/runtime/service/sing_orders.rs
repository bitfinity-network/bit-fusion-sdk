use std::collections::HashMap;

use bridge_did::{
    error::{BftResult, Error},
    op_id::OperationId,
    order::MintOrder,
};
use candid::CandidType;
use did::keccak;
use eth_signer::sign_strategy::TransactionSigner;
use serde::{Deserialize, Serialize};

use super::BridgeService;

pub trait MintOrderHandler {
    fn get_order(&self, id: OperationId) -> Option<MintOrder>;
    fn get_signer(&self) -> BftResult<impl TransactionSigner>;
    fn set_signed_order(&self, id: OperationId, signed: SignedOrders);
}

pub const MAX_MINT_ORDERS_IN_BATCH: usize = 16;

pub struct SignMintOrdersService<H: MintOrderHandler> {
    order_handler: H,
    orders: HashMap<OperationId, MintOrder>,
}

impl<H: MintOrderHandler> BridgeService for SignMintOrdersService<H> {
    async fn push_operation(&mut self, id: OperationId) -> BftResult<()> {
        let order = self
            .order_handler
            .get_order(id)
            .ok_or(Error::OperationNotFound(id))?;

        self.orders.insert(id, order);
        Ok(())
    }

    async fn run(&mut self) -> BftResult<()> {
        let orders_number = self.orders.len().min(MAX_MINT_ORDERS_IN_BATCH);
        let order_ops: Vec<(OperationId, MintOrder)> = self
            .orders
            .iter()
            .map(|(id, order)| (*id, order.clone()))
            .collect();

        let mut orders_data = Vec::with_capacity(orders_number * MintOrder::ENCODED_DATA_SIZE);
        for order_op in &order_ops {
            let encoded_order = order_op.1.encode();
            orders_data.extend_from_slice(&encoded_order.0);
        }

        let signer = self.order_handler.get_signer()?;
        let digest = keccak::keccak_hash(&orders_data);
        let signature = signer.sign_digest(digest.0 .0).await?;
        let signature_bytes: [u8; 65] = ethers_core::types::Signature::from(signature).into();

        let signed = SignedOrders {
            orders_data,
            signature: signature_bytes.to_vec(),
        };

        for order_op in order_ops {
            self.orders.remove(&order_op.0);
            self.order_handler
                .set_signed_order(order_op.0, signed.clone());
        }

        Ok(())
    }
}

pub const SIGNATURE_LEN: usize = 65;

#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub struct SignedOrders {
    pub orders_data: Vec<u8>,
    pub signature: Vec<u8>,
}
