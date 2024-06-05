use candid::{CandidType, Decode, Deserialize, Encode};
use did::H160;
use ic_stable_structures::stable_structures::{DefaultMemoryImpl, Memory};
use ic_stable_structures::{Bound, MultimapStructure, StableMultimap, Storable, VirtualMemory};
use serde::Serialize;
use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::memory::{DEPOSIT_STORE_MEMORY_ID, MEMORY_MANAGER};

type EthAddress = H160;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, CandidType, Deserialize, Serialize, Hash,
)]
pub struct DepositRequestId(u64);

impl DepositRequestId {
    fn next() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    fn nonce(&self) -> u32 {
        (self.0 % u32::MAX as u64) as u32
    }
}

impl Storable for DepositRequestId {
    fn to_bytes(&self) -> Cow<[u8]> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(u64::from_bytes(bytes))
    }

    const BOUND: Bound = <u64 as Storable>::BOUND;
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct DepositRequest<P>
where
    P: Clone + CandidType,
{
    pub request_id: DepositRequestId,
    pub request_payload: P,
    pub state: DepositState,
}

#[derive(Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum DepositState {
    Scheduled,
    MintOrdersCreated,
    MintOrdersSigned,
    MintOrdersSent,
    Minted,
    Rejected {
        reason: String,
    },
}

impl<P> Storable for DepositRequest<P>
where
    P: Clone + CandidType + for<'de> Deserialize<'de>,
{
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).expect("failed to encode DepositState"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode DepositState")
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl<P> DepositRequest<P>
where
    P: Clone + CandidType + for<'de> Deserialize<'de>
{
    pub fn state(&self) -> &DepositState {
        &self.state
    }
}

pub struct DepositStore<M, P>
where
    M: Memory,
    P: Clone + CandidType + for<'de> Deserialize<'de>,
{
    stable_store: StableMultimap<EthAddress, DepositRequestId, DepositRequest<P>, M>,
}

impl<P> DepositStore<VirtualMemory<DefaultMemoryImpl>, P>
where
    P: Clone + CandidType + for<'de> Deserialize<'de>
{
    pub fn get() -> Self {
        Self {
            stable_store: MEMORY_MANAGER.with(|mm| StableMultimap::new(mm.get(DEPOSIT_STORE_MEMORY_ID))),
        }
    }
}

impl<M, P> DepositStore<M, P>
where
    M: Memory,
    P: Clone + CandidType + for<'de> Deserialize<'de>,
{
    pub fn with_memory(memory: M) -> Self {
        Self {
            stable_store: StableMultimap::new(memory),
        }
    }

    pub fn new_request(&mut self, eth_address: &EthAddress, payload: P) -> DepositRequest<P> {
        let request = DepositRequest {
            request_id: DepositRequestId::next(),
            request_payload: payload,
            state: DepositState::Scheduled,
        };
        self.stable_store
            .insert(eth_address, &request.request_id, &request);

        request
    }

    pub fn get_request(&self, eth_dst_address: &EthAddress, request_id: DepositRequestId) -> Option<DepositRequest<P>> {
        self.stable_store.get(eth_dst_address, &request_id)
    }

    pub fn get_by_address(&self, eth_dst_address: &H160) -> Vec<DepositRequest<P>> {
        let mut requests: Vec<_> = self.stable_store.range(eth_dst_address).map(|(_, request)| request).collect();
        requests.sort_unstable_by(|a, b| a.request_id.cmp(&b.request_id).reverse());

        requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ic_stable_structures::{MultimapStructure, VectorMemory};
    use std::collections::HashSet;

    fn test_store() -> DepositStore<VectorMemory, ()> {
        DepositStore::with_memory(VectorMemory::default())
    }

    fn eth_address(seed: u8) -> H160 {
        H160::from([seed; H160::BYTE_SIZE])
    }

    #[test]
    fn nonce_should_increment_with_id() {
        const CHECKS_NUM: usize = 100;
        let id1 = DepositRequestId::next();
        for _ in 0..CHECKS_NUM {
            let id2 = DepositRequestId::next();
            assert_ne!(id1, id2);
            assert_ne!(id1.nonce(), id2.nonce());
        }
    }

    #[test]
    fn nonce_should_not_overflow() {
        NEXT_ID.store(u32::MAX as u64 * 3 - 3, Ordering::Relaxed);

        const CHECKS_NUM: usize = 100;
        let id1 = DepositRequestId::next();
        for _ in 0..CHECKS_NUM {
            let id2 = DepositRequestId::next();
            assert_ne!(id1, id2);
            assert_ne!(id1.nonce(), id2.nonce());
        }
    }

    #[test]
    fn new_request_creates_and_stores_new_state() {
        const COUNT: usize = 42;

        let mut store = test_store();
        let mut request_set = HashSet::new();
        for i in 0..COUNT {
            let request = store.new_request(&eth_address(i as u8), ());
            request_set.insert(request.request_id);
            assert_eq!(store.stable_store.len(), i + 1);
        }

        assert_eq!(request_set.len(), COUNT);
    }

    #[test]
    fn new_request_stores_request_parameters() {
        let mut store: DepositStore<_, u32> = DepositStore::with_memory(VectorMemory::default());
        const COUNT: u32 = 42;
        let mut requests = vec![];
        for payload in 0..COUNT {
            let request = store.new_request(&eth_address(payload as u8), payload);
            assert_eq!(request.request_payload, payload);
            requests.push(request);
        }

        for request in requests {
            assert_eq!(store.get_request(&eth_address(request.request_payload as u8), request.request_id).unwrap().request_payload, request.request_payload);
        }
    }

    #[test]
    fn new_request_initial_state() {
        let mut store = test_store();
        let request = store.new_request(&eth_address(1), ());
        assert_eq!(request.state, DepositState::Scheduled);
    }

    #[test]
    fn get_all_requests_for_address_empty() {
        let mut store = test_store();
        assert!(store.get_by_address(&eth_address(1)).is_empty());
    }

    #[test]
    fn get_all_requests_for_address_one() {
        const COUNT: u8 = 42;
        let mut store = test_store();
        for i in 0..COUNT {
            let _ = store.new_request(&eth_address(i), ());
        }

        for i in 0..COUNT {
            assert_eq!(store.get_by_address(&eth_address(i)).len(), 1);
        }
    }

    #[test]
    fn get_all_requests_for_address_multiple() {
        const COUNT: u8 = 7;
        const EACH: usize = 5;
        let mut store = test_store();
        for addr in 0..COUNT {
            for i in 0..EACH {
                let _ = store.new_request(&eth_address(addr), ());
            }
        }

        for addr in 0..COUNT {
            assert_eq!(store.get_by_address(&eth_address(addr)).len(), EACH);
        }
    }

    #[test]
    fn get_all_requests_for_address_ordered() {
       const COUNT: usize = 11;
        let mut store = test_store();
        for i in 0..COUNT {
            let _ = store.new_request(&eth_address(1), ());
        }

        let requests = store.get_by_address(&eth_address(1));
        for i in 1..COUNT {
            assert!(requests[i].request_id < requests[i - 1].request_id);
        }
    }
}
