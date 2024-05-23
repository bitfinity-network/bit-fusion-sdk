use std::borrow::Cow;
use std::rc::Rc;

use candid::types::{Type, TypeInner};
use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{BTreeMapStructure, Bound, StableBTreeMap, Storable, VirtualMemory};
use minter_contract_utils::mint_orders::MintOrders;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;
use ord_rs::{Brc20, Inscription, InscriptionId};
use serde::Serialize;

use crate::memory::{
    BRC20_STORE_MEMORY_ID, BURN_REQUEST_MEMORY_ID, MEMORY_MANAGER, MINT_ORDERS_MEMORY_ID,
};

const SRC_TOKEN: Id256 = Id256([0; 32]);

/// Keeps track of BRC20 inscriptions owned by the canister.
pub struct Brc20Store {
    inner: StableBTreeMap<Brc20Id, Brc20Token, VirtualMemory<DefaultMemoryImpl>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize)]
pub struct Brc20Id(pub InscriptionId);

#[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub struct Brc20Token(pub Brc20);

impl Storable for Brc20Id {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(&(self,)).expect("serialization failed"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, (Self,)).expect("deserialization failed").0
    }

    const BOUND: Bound = Bound::Unbounded;
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

impl CandidType for Brc20Id {
    fn _ty() -> Type {
        Type(Rc::new(TypeInner::Text))
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        serializer.serialize_text(&self.0.to_string())
    }
}

impl From<InscriptionId> for Brc20Id {
    fn from(id: InscriptionId) -> Self {
        Self(id)
    }
}

impl From<Brc20Id> for InscriptionId {
    fn from(id: Brc20Id) -> Self {
        id.0
    }
}

impl CandidType for Brc20Token {
    fn _ty() -> Type {
        Type(Rc::new(TypeInner::Text))
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        serializer.serialize_text(
            &self
                .0
                .encode()
                .expect("Failed to encode BRC20 as JSON string"),
        )
    }
}

impl From<Brc20> for Brc20Token {
    fn from(brc20: Brc20) -> Self {
        Self(brc20)
    }
}

impl From<Brc20Token> for Brc20 {
    fn from(token: Brc20Token) -> Self {
        token.0
    }
}

#[derive(Debug, Clone, Deserialize, CandidType)]
pub struct StorableBrc20 {
    pub token_id: Brc20Id,
    pub token: Brc20Token,
}

impl StorableBrc20 {
    pub fn brc20_iid(&self) -> InscriptionId {
        self.token_id.0
    }

    pub fn actual_brc20(self) -> Brc20 {
        self.token.0
    }
}

impl Default for Brc20Store {
    fn default() -> Self {
        Self {
            inner: StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(BRC20_STORE_MEMORY_ID))),
        }
    }
}

impl Brc20Store {
    /// Saves a list of parsed BRC20 inscriptions and their IDs to the store.
    pub fn write_all(&mut self, inscriptions: &[StorableBrc20]) {
        inscriptions.iter().for_each(|brc20| {
            self.inner.insert(brc20.token_id, brc20.token.clone());

            log::debug!(
                "Added BRC20 token with ID {:?} to the store",
                brc20.token_id
            );
        });
    }

    pub fn fetch_by_id(&self, iid: &str) -> Brc20 {
        let iid = InscriptionId::parse_from_str(iid).expect("Failed to InscriptionId from string");
        self.get_token_info(iid)
            .expect("No BRC20 token found for the specified ID")
            .0
            .clone()
    }

    /// Retrieves all BRC20 inscriptions in the store.
    pub fn read_all(&self) -> Vec<StorableBrc20> {
        self.inner
            .iter()
            .map(|(token_id, token)| StorableBrc20 { token_id, token })
            .collect()
    }

    pub(crate) fn has_inscription(&self, iid: &str) -> bool {
        let iid = InscriptionId::parse_from_str(iid)
            .expect("Failed to convert InscriptionId from string");
        self.get_token_info(iid).is_some()
    }

    pub fn remove(&mut self, iid: InscriptionId) -> Result<(), String> {
        match self.inner.remove(&Brc20Id(iid)) {
            Some(_v) => Ok(()),
            None => Err("Token not found in store".to_string()),
        }
    }

    fn get_token_info(&self, iid: InscriptionId) -> Option<Brc20Token> {
        self.inner.get(&Brc20Id(iid))
    }
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
