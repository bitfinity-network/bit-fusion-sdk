use bridge_did::id256::Id256;
use bridge_did::order::SignedMintOrder;
use bridge_utils::mint_orders::MintOrders;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::VirtualMemory;

use crate::memory::{MEMORY_MANAGER, MINT_ORDERS_MEMORY_ID};

pub struct MintOrdersStore(MintOrders<VirtualMemory<DefaultMemoryImpl>>);

const SRC_TOKEN: Id256 = Id256([0; 32]);

#[derive(Debug)]
pub struct Entry {
    pub sender: Id256,
    pub nonce: u32,
    pub mint_order: SignedMintOrder,
}

impl Default for MintOrdersStore {
    fn default() -> Self {
        Self(MintOrders::new(
            MEMORY_MANAGER.with(|mm| mm.get(MINT_ORDERS_MEMORY_ID)),
        ))
    }
}

impl MintOrdersStore {
    pub fn push(&mut self, sender: Id256, nonce: u32, mint_order: SignedMintOrder) {
        self.0.insert(sender, SRC_TOKEN, nonce, mint_order);
    }

    pub fn remove(&mut self, sender: Id256, nonce: u32) {
        self.0.remove(sender, SRC_TOKEN, nonce);
    }
}
