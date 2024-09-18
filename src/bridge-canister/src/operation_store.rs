//! Abstract stable storage for user-initiated operations in bridge canisters. It can be used
//! to track an operation status and retrieve all operations for a given user ETH wallet.

use std::borrow::Cow;

use bridge_did::op_id::OperationId;
use bridge_did::operation_log::{Memo, OperationLog};
use bridge_utils::common::Pagination;
use candid::{CandidType, Decode, Deserialize, Encode};
use did::H160;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{
    BTreeMapStructure, Bound, CachedStableBTreeMap, CellStructure, MultimapStructure,
    StableBTreeMap, StableCell, StableMultimap, Storable,
};

use crate::bridge::Operation;

const DEFAULT_CACHE_SIZE: u32 = 1000;
const DEFAULT_MAX_REQUEST_COUNT: u64 = 100_000;

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
    pub memo_operations_map: Mem,
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
    incomplete_operations: CachedStableBTreeMap<OperationId, OperationLog<P>, M>,
    operations_log: StableBTreeMap<OperationId, OperationLog<P>, M>,
    address_operation_map: StableBTreeMap<H160, OperationIdList, M>,
    memo_operation_map: StableMultimap<H160, Memo, OperationId, M>,
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
            memo_operation_map: StableMultimap::new(memory.memo_operations_map),
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
    pub fn new_operation(&mut self, payload: P, memo: Option<Memo>) -> OperationId {
        let id = self.next_operation_id();
        self.new_operation_with_id(id, payload, memo);
        id
    }

    /// Initializes a new operation with the given payload for the given ETH wallet address
    /// and stores it.
    pub fn new_operation_with_id(
        &mut self,
        id: OperationId,
        payload: P,
        memo: Option<Memo>,
    ) -> OperationId {
        let wallet_address = payload.evm_wallet_address();
        let is_complete = payload.is_complete();
        let log = OperationLog::new(payload, wallet_address.clone(), memo);

        log::trace!("Operation {id} is created.");

        if is_complete {
            self.move_to_log(id, log);
        } else {
            self.incomplete_operations.insert(id, log);
        }

        let mut ids = self
            .address_operation_map
            .get(&wallet_address)
            .unwrap_or_default();
        ids.0.push(id);
        self.address_operation_map
            .insert(wallet_address.0.into(), ids);

        if let Some(memo) = memo {
            self.memo_operation_map.insert(&wallet_address, &memo, id);
        }

        id
    }

    /// Retrieves an operation by its ID.
    pub fn get(&self, operation_id: OperationId) -> Option<P> {
        self.get_log(operation_id).map(|p| p.current_step().clone())
    }

    /// Returns log of an operation by its ID.
    pub fn get_log(&self, operation_id: OperationId) -> Option<OperationLog<P>> {
        self.incomplete_operations
            .get(&operation_id)
            .or_else(|| self.operations_log.get(&operation_id))
    }

    fn get_with_id(&self, operation_id: OperationId) -> Option<(OperationId, P)> {
        self.incomplete_operations
            .get(&operation_id)
            .or_else(|| self.operations_log.get(&operation_id))
            .map(|log| (operation_id, log.current_step().clone()))
    }

    /// Returns operation for the given address with the given nonce, if present.
    pub fn get_for_address_nonce(
        &self,
        dst_address: &H160,
        nonce: u32,
    ) -> Option<(OperationId, P)> {
        self.get_for_address(dst_address, None)
            .iter()
            .find(|(op_id, _)| op_id.nonce() == nonce)
            .cloned()
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

    /// Retrieve operations for the given memo.
    pub fn get_operation_by_memo_and_user(
        &self,
        memo: &Memo,
        user: &H160,
    ) -> Option<(OperationId, P)> {
        self.memo_operation_map
            .get(user, memo)
            .and_then(|id| self.get_with_id(id))
            .or(None)
    }

    /// Retrieve all memos for a given user_id in the store.
    pub fn get_memos_by_user_address(&self, user_id: &H160) -> Vec<Memo> {
        self.memo_operation_map
            .range(user_id)
            .map(|(memo, _)| (memo))
            .collect()
    }

    /// Update the payload of the operation with the given id. If no operation with the given ID
    /// is found, nothing is done (except an error message in the log).
    pub fn update(&mut self, operation_id: OperationId, payload: P) {
        let Some(mut log) = self.incomplete_operations.get(&operation_id) else {
            log::error!("Cannot update operation {operation_id} status: not found");
            return;
        };

        let is_complete = payload.is_complete();
        log.add_step(Ok(payload));

        if is_complete {
            self.move_to_log(operation_id, log);
        } else {
            self.incomplete_operations.insert(operation_id, log);
        }
    }

    pub fn update_with_err(&mut self, operation_id: OperationId, error_message: String) {
        let Some(mut log) = self.incomplete_operations.get(&operation_id) else {
            log::error!("Cannot update operation {operation_id} status: not found");
            return;
        };

        log.add_step(Err(error_message));
        self.incomplete_operations.insert(operation_id, log);
    }

    fn move_to_log(&mut self, operation_id: OperationId, log: OperationLog<P>) {
        self.incomplete_operations.remove(&operation_id);
        self.operations_log.insert(operation_id, log);

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
                .get(oldest.wallet_address())
                .unwrap_or_default();
            let count_before = ids.0.len();
            ids.0.retain(|stored_id| *stored_id != id);

            if ids.0.len() != count_before {
                if ids.0.is_empty() {
                    self.address_operation_map.remove(oldest.wallet_address());
                } else {
                    // We rewrite the value stored in stable memory with the updated value here
                    self.address_operation_map
                        .insert(oldest.wallet_address().clone(), ids);
                }
            }

            // Clean up the memos
            self.memo_operation_map
                .remove_partial(oldest.wallet_address());

            let memos_to_remove: Vec<_> = self
                .memo_operation_map
                .iter()
                .filter_map(|(address, memo, op_id)| (op_id == id).then_some((address, memo)))
                .collect();

            for (user, memo) in memos_to_remove {
                self.memo_operation_map.remove(&user, &memo);
            }

            log::trace!("Operation {id} and its associated memos removed from the store.");
        }
    }
}

#[cfg(test)]
mod tests {
    use bridge_did::error::BftResult;
    use bridge_did::event_data::*;
    use ic_exports::ic_kit::MockContext;
    use ic_stable_structures::VectorMemory;
    use serde::Serialize;

    use super::*;
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
            _ctx: RuntimeState<Self>,
            _event: MintedEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            None
        }

        async fn on_wrapped_token_burnt(
            _ctx: RuntimeState<Self>,
            _event: BurntEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            None
        }

        async fn on_minter_notification(
            _ctx: RuntimeState<Self>,
            _event: NotifyMinterEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            None
        }
    }

    fn test_store(max_operations: u64) -> OperationStore<VectorMemory, TestOp> {
        MockContext::new().inject();
        let memory = OperationsMemory {
            id_counter: VectorMemory::default(),
            incomplete_operations: VectorMemory::default(),
            operations_log: VectorMemory::default(),
            operations_map: VectorMemory::default(),
            memo_operations_map: VectorMemory::default(),
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
            store.new_operation(TestOp::complete(i as _), None);
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
            store.new_operation(TestOp::complete(0), None);
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
            store.new_operation(TestOp::complete(42), None);
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
            let id = store.new_operation(TestOp::new(42, i as _), None);
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
            ids.push(store.new_operation(TestOp::new(i as _, 1), None));
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

    #[test]
    fn test_get_operation_by_memo() {
        const COUNT: u64 = 42;

        let mut store = test_store(COUNT);

        for i in 0..COUNT {
            store.new_operation(TestOp::new(i as _, 1), Some([i as u8; 32]));
        }

        for i in 0..COUNT {
            assert_eq!(
                store
                    .get_operation_by_memo_and_user(&[i as u8; 32], &eth_address(i as u8))
                    .unwrap()
                    .1
                    .addr,
                i as u32
            );
        }
    }

    #[test]
    fn get_all_memos_by_user() {
        let mut store = test_store(10);

        for i in 0..10 {
            store.new_operation(TestOp::new(5, 1), Some([i as u8; 32]));
        }

        let user_id = eth_address(5);
        let memos = store.get_memos_by_user_address(&user_id);
        assert_eq!(memos.len(), 10);
    }

    #[test]
    fn test_memo_operations_are_cleared_when_evicted() {
        const LIMIT: u64 = 10;
        const COUNT: u64 = 40;

        let mut store = test_store(LIMIT);

        for i in 0..COUNT {
            store.new_operation(TestOp::new(i as _, 1), Some([i as u8; 32]));
        }

        // Ensure that the memo map is populated correctly
        for i in 0..COUNT {
            assert!(store
                .get_operation_by_memo_and_user(&[i as u8; 32], &eth_address(i as u8))
                .is_some());
        }

        // Let us mark all of them complete
        for i in 0..COUNT {
            store.update(
                store
                    .get_operation_by_memo_and_user(&[i as u8; 32], &eth_address(i as u8))
                    .unwrap()
                    .0,
                TestOp::complete(i as _),
            );
        }

        for i in COUNT..COUNT + 10 {
            store.new_operation(TestOp::new(i as _, 1), Some([i as u8; 32]));

            store.update(
                store
                    .get_operation_by_memo_and_user(&[i as u8; 32], &eth_address(i as u8))
                    .unwrap()
                    .0,
                TestOp::complete(i as _),
            );
        }

        // Try to get the old ones
        for i in 0..COUNT {
            assert!(store
                .get_operation_by_memo_and_user(&[i as u8; 32], &eth_address(i as u8))
                .is_none());
        }
    }
}
