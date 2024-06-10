//! Abstract stable storage for user-initiated operations in bridge canisters. It can be used
//! to track an operation status and retrieve all operations for a given user ETH wallet.
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};

use candid::{CandidType, Decode, Deserialize, Encode};
use did::H160;
use ic_stable_structures::{
    Bound, BTreeMapStructure, CachedStableBTreeMap, IterableSortedMapStructure, StableBTreeMap,
    Storable,
};
use ic_stable_structures::stable_structures::Memory;
use serde::Serialize;

const DEFAULT_CACHE_SIZE: u32 = 1000;
const DEFAULT_MAX_REQUEST_COUNT: u64 = 100_000;

thread_local! {
    static NEXT_ID: AtomicU64 = const { AtomicU64::new(0) };
}

/// Unique ID of an operation.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, CandidType, Deserialize, Serialize, Hash,
)]
pub struct MinterOperationId(u64);

impl MinterOperationId {
    fn next() -> Self {
        Self(NEXT_ID.with(|v| v.fetch_add(1, Ordering::Relaxed)))
    }

    /// Returns a unique `nonce` value for given operation ID.
    pub fn nonce(&self) -> u32 {
        (self.0 % u32::MAX as u64) as u32
    }
}

impl Storable for MinterOperationId {
    fn to_bytes(&self) -> Cow<[u8]> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(u64::from_bytes(bytes))
    }

    const BOUND: Bound = <u64 as Storable>::BOUND;
}

impl Display for MinterOperationId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
struct OperationStoreEntry<P>
where
    P: CandidType,
{
    dst_address: H160,
    payload: P,
}

impl<P> Storable for OperationStoreEntry<P>
where
    P: CandidType + Clone + for<'de> Deserialize<'de>,
{
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).expect("failed to encode deposit request"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode deposit request")
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(Default, Debug, Clone, CandidType, Deserialize)]
struct OperationIdList(Vec<MinterOperationId>);

impl Storable for OperationIdList {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).expect("failed to encode deposit request"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode deposit request")
    }

    const BOUND: Bound = Bound::Unbounded;
}

/// Parameters of the [`MinterOperationStore`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MinterOperationStoreOptions {
    max_operations_count: u64,
    cache_size: u32,
}

impl Default for MinterOperationStoreOptions {
    fn default() -> Self {
        Self {
            max_operations_count: DEFAULT_MAX_REQUEST_COUNT,
            cache_size: DEFAULT_CACHE_SIZE,
        }
    }
}

/// A structure to store user-initiated operations in IC stable memory.
///
/// Every operation in the store is attached to a ETH wallet address that initiated the operation.
/// And a list of operations for the given wallet can be retrieved by the [`get_for_address`] method.
///
/// It stores a limited number of latest operations and their information, dropping old operations.
/// The maximum number of operations stored can be configured with `options`.
pub struct MinterOperationStore<M, P>
where
    M: Memory,
    P: CandidType + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    operations: CachedStableBTreeMap<MinterOperationId, OperationStoreEntry<P>, M>,
    address_operation_map: StableBTreeMap<H160, OperationIdList, M>,
    max_operation_count: u64,
}

impl<M, P> MinterOperationStore<M, P>
where
    M: Memory,
    P: CandidType + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    /// Creates a new instance of the store.
    pub fn with_memory(
        requests_memory: M,
        map_memory: M,
        options: Option<MinterOperationStoreOptions>,
    ) -> Self {
        let options = options.unwrap_or_default();
        Self {
            operations: CachedStableBTreeMap::new(requests_memory, options.cache_size),
            address_operation_map: StableBTreeMap::new(map_memory),
            max_operation_count: options.max_operations_count,
        }
    }

    /// Initializes a new operation with the given payload for the given ETH wallet address
    /// and stores it.
    pub fn new_operation(&mut self, dst_address: H160, payload: P) -> MinterOperationId {
        let id = MinterOperationId::next();
        self.operations.insert(
            id,
            OperationStoreEntry {
                dst_address: dst_address.clone(),
                payload,
            },
        );

        let mut ids = self
            .address_operation_map
            .get(&dst_address)
            .unwrap_or_default();
        ids.0.push(id);
        self.address_operation_map.insert(dst_address, ids);

        if self.operations.len() > self.max_operation_count() {
            self.remove_oldest();
        }

        id
    }

    /// Retrieves an operation by its ID.
    pub fn get(&self, operation_id: MinterOperationId) -> Option<P> {
        self.operations
            .get(&operation_id)
            .map(|entry| entry.payload)
    }

    /// Retrieves all operations for the given ETH wallet address.
    pub fn get_for_address(&self, dst_address: &H160) -> Vec<(MinterOperationId, P)> {
        self.address_operation_map
            .get(dst_address)
            .unwrap_or_default()
            .0
            .into_iter()
            .filter_map(|id| {
                self.operations
                    .get(&id)
                    .map(|entry| (id, entry.payload))
            })
            .collect()
    }

    /// Update the payload of the operation with the given id. If no operation with the given ID
    /// is found, nothing is done (except an error message in the log).
    pub fn update(&mut self, operation_id: MinterOperationId, payload: P) {
        let Some(mut entry) = self.operations.get(&operation_id) else {
            log::error!("Cannot update operation {operation_id} status: not found");
            return;
        };

        entry.payload = payload;
        self.operations.insert(operation_id, entry);
    }

    fn max_operation_count(&self) -> u64 {
        self.max_operation_count
    }

    fn remove_oldest(&mut self) {
        if let Some((id, oldest)) = self.operations.iter().next() {
            self.operations.remove(&id);
            let mut ids = self
                .address_operation_map
                .get(&oldest.dst_address)
                .unwrap_or_default();
            let count_before = ids.0.len();
            ids.0.retain(|stored_id| *stored_id != id);

            if ids.0.len() != count_before {
                if ids.0.is_empty() {
                    self.address_operation_map.remove(&oldest.dst_address);
                } else {
                    self.address_operation_map.insert(oldest.dst_address, ids);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ic_stable_structures::VectorMemory;

    use super::*;

    fn test_store(max_operations: u64) -> MinterOperationStore<VectorMemory, u32> {
        MinterOperationStore::with_memory(VectorMemory::default(), VectorMemory::default(), Some(MinterOperationStoreOptions {
            max_operations_count: max_operations,
            cache_size: DEFAULT_CACHE_SIZE,
        }))
    }

    fn eth_address(seed: u8) -> H160 {
        H160::from([seed; H160::BYTE_SIZE])
    }

    #[test]
    fn nonce_should_increment_with_id() {
        const CHECKS_NUM: usize = 100;
        let id1 = MinterOperationId::next();
        for _ in 0..CHECKS_NUM {
            let id2 = MinterOperationId::next();
            assert_ne!(id1, id2);
            assert_ne!(id1.nonce(), id2.nonce());
        }
    }

    #[test]
    fn nonce_should_not_overflow() {
        NEXT_ID.with(|v| v.store(u32::MAX as u64 * 3 - 3, Ordering::Relaxed));

        const CHECKS_NUM: usize = 100;
        let id1 = MinterOperationId::next();
        for _ in 0..CHECKS_NUM {
            let id2 = MinterOperationId::next();
            assert_ne!(id1, id2);
            assert_ne!(id1.nonce(), id2.nonce());
        }
    }

    #[test]
    fn operations_limit() {
        const LIMIT: u64 = 10;
        const COUNT: u64 = 42;

        let mut store = test_store(LIMIT);

        for i in 0..COUNT {
            store.new_operation(eth_address(i as u8), i as u32);
        }

        assert_eq!(store.operations.len(), LIMIT);
        assert_eq!(store.address_operation_map.len(), LIMIT);

        for i in 0..(COUNT - LIMIT) {
            assert!(store.get_for_address(&eth_address(i as u8)).is_empty());
        }

        for i in (COUNT - LIMIT)..COUNT {
            assert_eq!(store.get_for_address(&eth_address(i as u8)).len(), 1);
        }
    }

    #[test]
    fn operations_limit_with_same_address() {
        const LIMIT: u64 = 10;
        const COUNT: u64 = 42;

        let mut store = test_store(LIMIT);

        for i in 0..COUNT {
            store.new_operation(eth_address(1), i as u32);
        }

        assert_eq!(store.operations.len(), LIMIT);
        assert_eq!(store.address_operation_map.len(), 1);

        assert_eq!(store.get_for_address(&eth_address(1)).len(), LIMIT as usize);
    }
}
