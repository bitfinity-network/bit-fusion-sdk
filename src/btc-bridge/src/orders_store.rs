use bridge_did::id256::Id256;
use bridge_did::order::SignedMintOrder;
use bridge_utils::mint_orders::MintOrders;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{BTreeMapStructure as _, StableBTreeMap, VirtualMemory};

use crate::memory::{MEMORY_MANAGER, MINT_ORDERS_MEMORY_ID, MINT_ORDERS_NONCES_MEMORY_ID};

pub struct MintOrdersStore {
    /// contains the association between the sender and the last nonce used by the sender
    nonce_storage: StableBTreeMap<Id256, u32, VirtualMemory<DefaultMemoryImpl>>,
    orders: MintOrders<VirtualMemory<DefaultMemoryImpl>>,
}

const SRC_TOKEN: Id256 = Id256([0; 32]);

#[derive(Debug)]
pub struct Entry {
    pub sender: Id256,
    pub nonce: u32,
    pub mint_order: SignedMintOrder,
}

impl Default for MintOrdersStore {
    fn default() -> Self {
        Self {
            nonce_storage: StableBTreeMap::new(
                MEMORY_MANAGER.with(|mm| mm.get(MINT_ORDERS_NONCES_MEMORY_ID)),
            ),
            orders: MintOrders::new(MEMORY_MANAGER.with(|mm| mm.get(MINT_ORDERS_MEMORY_ID))),
        }
    }
}

impl MintOrdersStore {
    /// Stores the mint order and returns the nonce for the mint order
    pub fn push(&mut self, sender: Id256, nonce: u32, mint_order: SignedMintOrder) {
        self.orders.insert(sender, SRC_TOKEN, nonce, mint_order);
        self.nonce_storage.insert(sender, nonce);
    }

    /// Remove the mint order with the provided nonce for the provided sender
    pub fn remove(&mut self, sender: Id256, nonce: u32) {
        self.orders.remove(sender, SRC_TOKEN, nonce);
    }

    /// Returns the next nonce for the provided sender.
    ///
    /// If the sender has not sent any mint orders, the nonce is 0.
    pub fn next_nonce(&self, sender: &Id256) -> u32 {
        self.nonce_storage
            .get(sender)
            .map(|nonce| nonce + 1)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod test {
    use did::H160;

    use super::*;

    #[test]
    fn test_should_get_and_update_next_nonce() {
        const ORDER: [u8; 334] = [0u8; 334];
        let mut storage = MintOrdersStore::default();

        let alice_address = Id256::from_evm_address(
            &H160::from_hex_str("0xf5e47fb8a65dcab9e27b11067b0ff474dcc9c5f6").unwrap(),
            1,
        );
        let bob_address = Id256::from_evm_address(
            &H160::from_hex_str("0x97b39ccf3b90fb0a7602f9cf878e8622b4bf68c1").unwrap(),
            1,
        );

        assert_eq!(storage.next_nonce(&alice_address), 0);
        assert_eq!(storage.next_nonce(&bob_address), 0);

        // store order
        storage.push(alice_address, 0, SignedMintOrder(ORDER));
        assert_eq!(storage.next_nonce(&alice_address), 1);
        assert_eq!(storage.next_nonce(&bob_address), 0);

        // get order nonce
        assert!(storage.orders.get(alice_address, SRC_TOKEN, 0).is_some());
    }
}
