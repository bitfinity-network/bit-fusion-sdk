//! Abstract stable storage for user-initiated operations in bridge canisters. It can be used
//! to track an operation status and retrieve all operations for a given user ETH wallet.

use std::borrow::Cow;

use bridge_did::op_id::OperationId;
use bridge_utils::common::Pagination;
use candid::{CandidType, Decode, Deserialize, Encode};
use did::H160;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{
    BTreeMapStructure, Bound, CachedStableBTreeMap, CellStructure, StableBTreeMap, StableCell,
    Storable,
};

use crate::bridge::Operation;

const DEFAULT_CACHE_SIZE: u32 = 1000;
const DEFAULT_MAX_REQUEST_COUNT: u64 = 100_000;

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
struct OperationIdList(Vec<OperationId>);

impl Storable for OperationIdList {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).expect("failed to encode deposit request"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode deposit request")
    }

    const BOUND: Bound = Bound::Unbounded;
}

/// Parameters of the [`OperationStore`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct OperationStoreOptions {
    max_operations_count: u64,
    cache_size: u32,
}

impl Default for OperationStoreOptions {
    fn default() -> Self {
        Self {
            max_operations_count: DEFAULT_MAX_REQUEST_COUNT,
            cache_size: DEFAULT_CACHE_SIZE,
        }
    }
}

/// Memory objects to store operations.
pub struct OperationsMemory<Mem> {
    pub id_counter: Mem,
    pub incomplete_operations: Mem,
    pub operations_log: Mem,
    pub operations_map: Mem,
}

/// A structure to store user-initiated operations in IC stable memory.
///
/// Every operation in the store is attached to a ETH wallet address that initiated the operation.
/// And a list of operations for the given wallet can be retrieved by the [`get_for_address`] method.
///
/// It stores a limited number of latest operations and their information, dropping old operations.
/// The maximum number of operations stored can be configured with `options`.
pub struct OperationStore<M, P>
where
    M: Memory,
    P: Operation,
{
    operation_id_counter: StableCell<u64, M>,
    incomplete_operations: CachedStableBTreeMap<OperationId, OperationStoreEntry<P>, M>,
    operations_log: StableBTreeMap<OperationId, OperationStoreEntry<P>, M>,
    address_operation_map: StableBTreeMap<H160, OperationIdList, M>,
    max_operation_log_size: u64,
}

impl<M, P> OperationStore<M, P>
where
    M: Memory,
    P: Operation,
{
    /// Creates a new instance of the store.
    pub fn with_memory(
        memory: OperationsMemory<M>,
        options: Option<OperationStoreOptions>,
    ) -> Self {
        let options = options.unwrap_or_default();
        Self {
            operation_id_counter: StableCell::new(memory.id_counter, 0)
                .expect("failed to initialize operation id counter"),
            incomplete_operations: CachedStableBTreeMap::new(
                memory.incomplete_operations,
                options.cache_size,
            ),
            operations_log: StableBTreeMap::new(memory.operations_log),
            address_operation_map: StableBTreeMap::new(memory.operations_map),
            max_operation_log_size: options.max_operations_count,
        }
    }

    /// Returns next OperationId.
    fn next_operation_id(&mut self) -> OperationId {
        let current = *self.operation_id_counter.get();

        self.operation_id_counter
            .set(current + 1)
            .expect("failed to update operation id counter");

        OperationId::new(current)
    }

    /// Initializes a new operation with the given payload for the given ETH wallet address
    /// and stores it.
    pub fn new_operation(&mut self, payload: P) -> OperationId {
        let id = self.next_operation_id();
        let dst_address = payload.evm_wallet_address();
        let entry = OperationStoreEntry {
            dst_address: dst_address.clone(),
            payload,
        };

        log::trace!("Operation {id} is created.");

        if entry.payload.is_complete() {
            self.move_to_log(id, entry);
        } else {
            self.incomplete_operations.insert(id, entry);
        }

        let mut ids = self
            .address_operation_map
            .get(&dst_address)
            .unwrap_or_default();
        ids.0.push(id);
        self.address_operation_map.insert(dst_address, ids);

        id
    }

    /// Retrieves an operation by its ID.
    pub fn get(&self, operation_id: OperationId) -> Option<P> {
        self.get_with_id(operation_id).map(|(_, p)| p)
    }

    fn get_with_id(&self, operation_id: OperationId) -> Option<(OperationId, P)> {
        self.incomplete_operations
            .get(&operation_id)
            .or_else(|| self.operations_log.get(&operation_id))
            .map(|entry| (operation_id, entry.payload))
    }

    /// Retrieves all operations for the given ETH wallet address,
    /// starting from `offset` returning a max of `count` items
    /// If `offset` is `None`, it starts from the beginning.
    /// If `count` is `None`, it returns all operations.
    pub fn get_for_address(
        &self,
        dst_address: &H160,
        pagination: Option<Pagination>,
    ) -> Vec<(OperationId, P)> {
        log::trace!("Operation store contains {} active operations, {} operations in log, {} entries in the map. Value for address {}: {:?}", self.incomplete_operations.len(), self.operations_log.len(), self.address_operation_map.len(), hex::encode(dst_address.0), self.address_operation_map.get(dst_address));

        let offset = pagination.as_ref().map(|p| p.offset).unwrap_or(0);
        let count = pagination.map(|p| p.count).unwrap_or(usize::MAX);

        self.address_operation_map
            .get(dst_address)
            .unwrap_or_default()
            .0
            .into_iter()
            .filter_map(|id| self.get_with_id(id))
            .skip(offset)
            .take(count)
            .collect()
    }

    /// Update the payload of the operation with the given id. If no operation with the given ID
    /// is found, nothing is done (except an error message in the log).
    pub fn update(&mut self, operation_id: OperationId, payload: P) {
        let Some(mut entry) = self.incomplete_operations.get(&operation_id) else {
            log::error!("Cannot update operation {operation_id} status: not found");
            return;
        };

        entry.payload = payload;

        if entry.payload.is_complete() {
            self.move_to_log(operation_id, entry);
        } else {
            self.incomplete_operations.insert(operation_id, entry);
        }
    }

    fn move_to_log(&mut self, operation_id: OperationId, entry: OperationStoreEntry<P>) {
        self.incomplete_operations.remove(&operation_id);
        self.operations_log.insert(operation_id, entry);

        log::trace!("Operation {operation_id} is marked as complete and moved to the log.");

        if self.operations_log.len() > self.max_operation_log_size() {
            self.remove_oldest();
        }
    }

    fn max_operation_log_size(&self) -> u64 {
        self.max_operation_log_size
    }

    fn remove_oldest(&mut self) {
        if let Some((id, oldest)) = self.operations_log.iter().next() {
            self.operations_log.remove(&id);
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

            log::trace!("Operation {id} is evicted from the operation log");
        }
    }
}

#[cfg(test)]
mod tests {
    use bridge_did::error::BftResult;
    use ic_stable_structures::VectorMemory;
    use serde::Serialize;

    use super::*;
    use crate::bridge::OperationContext;
    use crate::runtime::RuntimeState;

    #[derive(Debug, Copy, Clone, Serialize, Deserialize, CandidType)]
    struct TestOp {
        pub addr: u32,
        pub stage: u32,
    }

    const COMPLETE: u32 = u32::MAX;

    impl TestOp {
        pub fn new(addr: u32, stage: u32) -> Self {
            Self { addr, stage }
        }

        pub fn complete(addr: u32) -> Self {
            Self {
                addr,
                stage: COMPLETE,
            }
        }
    }

    impl Operation for TestOp {
        fn is_complete(&self) -> bool {
            self.stage == COMPLETE
        }

        async fn progress(self, _id: OperationId, _ctx: RuntimeState<Self>) -> BftResult<Self> {
            todo!()
        }

        fn evm_wallet_address(&self) -> H160 {
            eth_address(self.addr as _)
        }

        async fn on_wrapped_token_minted(
            _ctx: impl OperationContext,
            _event: bridge_utils::bft_events::MintedEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            None
        }

        async fn on_wrapped_token_burnt(
            _ctx: impl OperationContext,
            _event: bridge_utils::bft_events::BurntEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            None
        }

        async fn on_minter_notification(
            _ctx: impl OperationContext,
            _event: bridge_utils::bft_events::NotifyMinterEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            None
        }
    }

    fn test_store(max_operations: u64) -> OperationStore<VectorMemory, TestOp> {
        let memory = OperationsMemory {
            id_counter: VectorMemory::default(),
            incomplete_operations: VectorMemory::default(),
            operations_log: VectorMemory::default(),
            operations_map: VectorMemory::default(),
        };
        OperationStore::with_memory(
            memory,
            Some(OperationStoreOptions {
                max_operations_count: max_operations,
                cache_size: DEFAULT_CACHE_SIZE,
            }),
        )
    }

    fn eth_address(seed: u8) -> H160 {
        H160::from([seed; H160::BYTE_SIZE])
    }

    #[test]
    fn operations_log_limit() {
        const LIMIT: u64 = 10;
        const COUNT: u64 = 42;

        let mut store = test_store(LIMIT);

        for i in 0..COUNT {
            store.new_operation(TestOp::complete(i as _));
        }

        assert_eq!(store.operations_log.len(), LIMIT);
        assert_eq!(store.address_operation_map.len(), LIMIT);

        for i in 0..(COUNT - LIMIT) {
            assert!(store
                .get_for_address(&eth_address(i as u8), None)
                .is_empty());
        }

        for i in (COUNT - LIMIT)..COUNT {
            assert_eq!(store.get_for_address(&eth_address(i as u8), None).len(), 1,);
        }
    }

    #[test]
    fn should_get_page_for_operations() {
        const LIMIT: u64 = 100;
        const COUNT: u64 = 42;

        let mut store = test_store(LIMIT);

        for _ in 0..COUNT {
            store.new_operation(TestOp::complete(0));
        }

        assert_eq!(store.operations_log.len(), COUNT);

        // No offset, with count
        let page = store.get_for_address(&eth_address(0), Some(Pagination::new(0, 10)));
        assert_eq!(page.len(), 10);
        // No offset with count > total
        let page = store.get_for_address(&eth_address(0), Some(Pagination::new(0, 120)));
        assert_eq!(page.len() as u64, COUNT);

        // Offset with count
        let page = store.get_for_address(&eth_address(0), Some(Pagination::new(20, 15)));
        assert_eq!(page.len(), 15);

        // Offset with count beyond total
        let page = store.get_for_address(&eth_address(0), Some(Pagination::new(100, 10)));
        assert!(page.is_empty());
    }

    #[test]
    fn operations_limit_with_same_address() {
        const LIMIT: u64 = 10;
        const COUNT: u64 = 42;

        let mut store = test_store(LIMIT);

        for _ in 0..COUNT {
            store.new_operation(TestOp::complete(42));
        }

        assert_eq!(store.operations_log.len(), LIMIT);
        assert_eq!(store.address_operation_map.len(), 1);

        assert_eq!(
            store
                .get_for_address(&eth_address(42), Some(Pagination::new(0, 10)))
                .len(),
            LIMIT as usize
        );
    }

    #[test]
    fn incomplete_operations_are_not_removed() {
        const LIMIT: u64 = 10;
        const COUNT: u64 = 42;

        let mut store = test_store(LIMIT);

        for i in 0..COUNT {
            let id = store.new_operation(TestOp::new(42, i as _));
            store.update(id, TestOp::new(42, (i + 1) as _));
        }

        assert_eq!(store.operations_log.len(), 0);
        assert_eq!(store.incomplete_operations.len(), COUNT);
        assert_eq!(store.address_operation_map.len(), 1);

        assert_eq!(
            store
                .get_for_address(&eth_address(42), Some(Pagination::new(0, COUNT as usize)))
                .len(),
            COUNT as usize
        );
    }

    #[test]
    fn operations_are_moved_to_log_on_completion() {
        const LIMIT: u64 = 10;
        const COUNT: u64 = 42;

        let mut store = test_store(LIMIT);

        let mut ids = vec![];
        for i in 0..COUNT {
            ids.push(store.new_operation(TestOp::new(i as _, 1)));
        }

        for id in ids {
            let count_before = store.incomplete_operations.len();
            store.update(id, TestOp::complete(id.nonce()));
            let count_after = store.incomplete_operations.len();
            assert_eq!(count_after, count_before - 1);
        }

        assert_eq!(store.operations_log.len(), LIMIT);
        assert_eq!(store.incomplete_operations.len(), 0);
        assert_eq!(store.address_operation_map.len(), LIMIT);
    }
}
