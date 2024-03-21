use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

#[derive(Default, Debug)]
pub struct MintOrdersStore(pub Vec<Entry>);

#[derive(Debug)]
pub struct Entry {
    pub sender: Id256,
    pub nonce: u32,
    pub mint_order: SignedMintOrder,
}

impl MintOrdersStore {
    pub fn push(&mut self, sender: Id256, nonce: u32, mint_order: SignedMintOrder) {
        // todo: check for length, duplicates?
        self.0.push(Entry {
            sender,
            nonce,
            mint_order,
        })
    }

    pub fn remove(&mut self, sender: Id256, nonce: u32) {
        // todo: check if it is possible to have multiple orders with same values?
        self.0
            .retain(|entry| entry.sender != sender && entry.nonce != nonce)
    }
}
