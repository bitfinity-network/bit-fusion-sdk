use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

#[derive(Default, Debug)]
pub struct OrdersStore(pub Vec<Entry>);

#[derive(Debug)]
pub struct Entry {
    pub sender: Id256,
    pub nonce: u32,
    pub mint_order: SignedMintOrder,
}

impl OrdersStore {
    pub fn push_mint_order(&mut self, sender: Id256, nonce: u32, mint_order: SignedMintOrder) {
        // todo: check for length, duplicates?
        self.0.push(Entry {
            sender,
            nonce,
            mint_order,
        })
    }
}
