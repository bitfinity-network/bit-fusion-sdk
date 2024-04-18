use std::borrow::Cow;
use std::cmp::Ordering;

use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{BTreeMapStructure, Bound, StableBTreeMap, Storable, VirtualMemory};
use minter_contract_utils::mint_orders::MintOrders;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

use crate::memory::{
    BRC20_STORE_MEMORY_ID, BURN_REQUEST_MEMORY_ID, MEMORY_MANAGER, MINT_ORDERS_MEMORY_ID,
};

const SRC_TOKEN: Id256 = Id256([0; 32]);

pub struct Brc20Store {
    inner: StableBTreeMap<RevealTxId, Brc20TokenInfo, VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for Brc20Store {
    fn default() -> Self {
        Self {
            inner: StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(BRC20_STORE_MEMORY_ID))),
        }
    }
}

impl Brc20Store {
    pub(crate) fn get_token_info(&self, txid: &str) -> Option<Brc20TokenInfo> {
        self.inner.get(&RevealTxId(txid.to_string()))
    }

    pub(crate) fn insert(&mut self, token_info: Brc20TokenInfo) {
        self.inner
            .insert(token_info.tx_id.clone(), token_info.clone());
    }

    pub fn remove(&mut self, txid: String) -> Result<(), String> {
        match self.inner.remove(&RevealTxId(txid)) {
            Some(_v) => Ok(()),
            None => Err("Token not found in store".to_string()),
        }
    }

    pub(crate) fn has_inscription(&self, txid: &str) -> bool {
        self.get_token_info(txid).is_some()
    }
}

/// Represents the reveal transaction ID of the BRC-20 inscription.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RevealTxId(pub(crate) String);

impl Storable for RevealTxId {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = self.0.to_bytes().to_vec();
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(String::from_utf8(bytes.to_vec()).expect("Failed to convert bytes to String"))
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl PartialOrd<Self> for RevealTxId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RevealTxId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Brc20TokenInfo {
    pub(crate) tx_id: RevealTxId,
    pub(crate) ticker: String,
    pub(crate) holder: String,
}

impl Storable for Brc20TokenInfo {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut bytes = self.tx_id.0.as_bytes().to_vec();
        bytes.append(&mut self.ticker.as_bytes().to_vec());
        bytes.append(&mut self.holder.as_bytes().to_vec());

        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let tx_id =
            RevealTxId(String::from_utf8(bytes[0..32].to_vec()).expect("Invalid bytes length"));
        let ticker = String::from_utf8(bytes[32..36].to_vec()).expect("Invalid bytes length");
        let holder = String::from_utf8(bytes[36..].to_vec()).expect("Invalid bytes length");

        Self {
            tx_id,
            ticker,
            holder,
        }
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
    reveal_txid: String,
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
