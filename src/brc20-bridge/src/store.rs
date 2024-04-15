use std::borrow::Cow;
use std::cmp::Ordering;

use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{BTreeMapStructure, Bound, StableBTreeMap, Storable, VirtualMemory};
use minter_contract_utils::mint_orders::MintOrders;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;
use ord_rs::{Brc20, Inscription};

use crate::memory::{
    BRC20_INSCRIPTIONS_MEMORY_ID, BURN_REQUEST_MEMORY_ID, MEMORY_MANAGER, MINT_ORDERS_MEMORY_ID,
};

const SRC_TOKEN: Id256 = Id256([0; 32]);

pub struct Brc20Store {
    inner: StableBTreeMap<Brc20Hash, Brc20Inscription, VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for Brc20Store {
    fn default() -> Self {
        Self {
            inner: StableBTreeMap::new(
                MEMORY_MANAGER.with(|mm| mm.get(BRC20_INSCRIPTIONS_MEMORY_ID)),
            ),
        }
    }
}

impl Brc20Store {
    pub fn insert(&mut self, inscriptions: &[Brc20]) {
        for brc20 in inscriptions {
            let digest = sha256(&brc20.encode().expect("Failed to encode BRC-20 as string"));

            self.inner
                .insert(Brc20Hash(digest), Brc20Inscription(brc20.clone()));
        }
    }
}

fn sha256(input: &String) -> String {
    use sha2::Digest;

    let mut hasher = sha2::Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Represents the hex-encoded SHA2-256 digest of the BRC-20 inscription.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Brc20Hash(String);

impl Storable for Brc20Hash {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = self.0.to_bytes().to_vec();
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let brc20 = String::from_utf8(bytes.to_vec()).expect("Failed to convert bytes to String");
        Self(brc20)
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl PartialOrd<Self> for Brc20Hash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Brc20Hash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

/// Represents the full BRC-20 inscription object.
#[derive(Debug, Clone, Eq, PartialEq)]
struct Brc20Inscription(Brc20);

impl From<Brc20> for Brc20Inscription {
    fn from(inscription: Brc20) -> Self {
        Self(inscription)
    }
}

impl Storable for Brc20Inscription {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = self
            .0
            .encode()
            .expect("Failed to encode BRC20")
            .to_bytes()
            .to_vec();

        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(Brc20::parse(&bytes).expect("Failed to parse BRC20"))
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
    pub fn insert(&mut self, request_id: BurnRequestId, address: String, amount: u64) {
        self.inner.insert(
            request_id,
            BurnRequestInfo {
                address,
                amount,
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
    amount: u64,
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
