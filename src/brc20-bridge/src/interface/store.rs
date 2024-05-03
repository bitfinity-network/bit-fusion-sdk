use std::borrow::Cow;

use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{BTreeMapStructure, Bound, StableBTreeMap, Storable, VirtualMemory};
use minter_contract_utils::mint_orders::MintOrders;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;
use serde::Serialize;

use crate::memory::{
    BRC20_STORE_MEMORY_ID, BURN_REQUEST_MEMORY_ID, MEMORY_MANAGER, MINT_ORDERS_MEMORY_ID,
};

const SRC_TOKEN: Id256 = Id256([0; 32]);

pub type RevealTxId = String;

pub struct Brc20Store {
    inner: StableBTreeMap<RevealTxId, Brc20Token, VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for Brc20Store {
    fn default() -> Self {
        Self {
            inner: StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(BRC20_STORE_MEMORY_ID))),
        }
    }
}

impl Brc20Store {
    pub fn get_token_info(&self, txid: &str) -> Option<Brc20Token> {
        self.inner.get(&txid.to_string())
    }

    pub fn insert(&mut self, token_info: Brc20Token) {
        self.inner
            .insert(token_info.tx_id.clone(), token_info.clone());
    }

    pub fn remove(&mut self, txid: String) -> Result<(), String> {
        match self.inner.remove(&txid) {
            Some(_v) => Ok(()),
            None => Err("Token not found in store".to_string()),
        }
    }

    pub(crate) fn has_inscription(&self, txid: &str) -> bool {
        self.get_token_info(txid).is_some()
    }
}

#[derive(Debug, CandidType, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub struct Brc20Token {
    pub tx_id: RevealTxId,
    pub ticker: String,
    pub holder: String,
}

impl Storable for Brc20Token {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(&(self,)).expect("serialization failed"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, (Self,)).expect("deserialization failed").0
    }

    const BOUND: Bound = Bound::Unbounded;
}

pub struct MintOrdersStore(MintOrders<VirtualMemory<DefaultMemoryImpl>>);

impl Default for MintOrdersStore {
    fn default() -> Self {
        Self(MintOrders::new(
            MEMORY_MANAGER.with(|mm| mm.get(MINT_ORDERS_MEMORY_ID)),
        ))
    }
}

impl MintOrdersStore {
    pub fn push(&mut self, sender: Id256, nonce: u32, mint_order: SignedMintOrder) {
        self.0.insert(sender, SRC_TOKEN, nonce, &mint_order);
    }

    pub fn remove(&mut self, sender: Id256, nonce: u32) {
        self.0.remove(sender, SRC_TOKEN, nonce);
    }
}

pub type BurnRequestId = u32;

pub struct BurnRequestStore {
    inner: StableBTreeMap<BurnRequestId, BurnRequestInfo, VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for BurnRequestStore {
    fn default() -> Self {
        Self {
            inner: StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(BURN_REQUEST_MEMORY_ID))),
        }
    }
}

impl BurnRequestStore {
    pub fn insert(&mut self, request_id: BurnRequestId, address: String, reveal_txid: String) {
        self.inner.insert(
            request_id,
            BurnRequestInfo {
                address,
                reveal_txid,
                is_transferred: false,
            },
        );
    }

    pub fn remove(&mut self, request_id: BurnRequestId) {
        self.inner.remove(&request_id);
    }

    pub fn set_transferred(&mut self, request_id: BurnRequestId) {
        if let Some(v) = self.inner.remove(&request_id) {
            self.inner.insert(
                request_id,
                BurnRequestInfo {
                    is_transferred: true,
                    ..v
                },
            );
        }
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
struct BurnRequestInfo {
    address: String,
    reveal_txid: RevealTxId,
    is_transferred: bool,
}

impl Storable for BurnRequestInfo {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(&(self,)).expect("serialization failed"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, (Self,)).expect("deserialization failed").0
    }

    const BOUND: Bound = Bound::Unbounded;
}
