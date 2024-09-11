use std::cell::RefCell;
use std::collections::HashMap;

use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;
use bridge_did::order::{MintOrder, SignedOrder, SignedOrders};
use did::keccak;
use eth_signer::sign_strategy::TransactionSigner;

use super::BridgeService;

pub trait MintOrderHandler {
    /// Get signer to sign mint orders batch.
    fn get_signer(&self) -> BftResult<impl TransactionSigner>;

    /// Get mint order by the OperationId.
    fn get_order(&self, id: OperationId) -> Option<MintOrder>;

    /// Set signed mint orders data to the given operation.
    fn set_signed_order(&self, id: OperationId, signed: SignedOrder);
}

pub const MAX_MINT_ORDERS_IN_BATCH: usize = 16;

/// Service to sign mint order batches.
pub struct SignMintOrdersService<H: MintOrderHandler> {
    order_handler: H,
    orders: RefCell<HashMap<OperationId, MintOrder>>,
}

impl<H: MintOrderHandler> SignMintOrdersService<H> {
    /// Creates new mint order signing service.
    pub fn new(order_handler: H) -> Self {
        Self {
            order_handler,
            orders: Default::default(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<H: MintOrderHandler> BridgeService for SignMintOrdersService<H> {
    fn push_operation(&self, id: OperationId) -> BftResult<()> {
        let order = self
            .order_handler
            .get_order(id)
            .ok_or(Error::OperationNotFound(id))?;

        self.orders.borrow_mut().insert(id, order);
        Ok(())
    }

    async fn run(&self) -> BftResult<()> {
        let orders_number = self.orders.borrow().len().min(MAX_MINT_ORDERS_IN_BATCH);

        log::trace!("Singing batch of {orders_number} mint orders.");

        let order_ops: Vec<(OperationId, MintOrder)> = self
            .orders
            .borrow()
            .iter()
            .map(|(id, order)| (*id, order.clone()))
            .collect();

        let mut orders_data = Vec::with_capacity(orders_number * MintOrder::ENCODED_DATA_SIZE);
        for order_op in &order_ops {
            let encoded_order = order_op.1.encode();
            orders_data.extend_from_slice(&encoded_order);
        }

        let signer = self.order_handler.get_signer()?;
        let digest = keccak::keccak_hash(&orders_data);
        let signature = signer.sign_digest(digest.0 .0).await?;
        let signature_bytes: [u8; 65] = ethers_core::types::Signature::from(signature).into();

        log::trace!("Batch of {orders_number} mint orders signed");

        let signed_orders = SignedOrders {
            orders_data,
            signature: signature_bytes.to_vec(),
        };

        for (idx, order_op) in order_ops.into_iter().enumerate() {
            self.orders.borrow_mut().remove(&order_op.0);
            let signed_order = SignedOrder::new(signed_orders.clone(), idx)
                .expect("index inside the signed orders list");
            self.order_handler
                .set_signed_order(order_op.0, signed_order);
        }

        log::trace!("Operations updated for batch of {orders_number} mint orders");

        Ok(())
    }
}
