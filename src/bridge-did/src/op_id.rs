use core::fmt;
use std::borrow::Cow;

use candid::CandidType;
use ic_stable_structures::{Bound, Storable};
use serde::{Deserialize, Serialize};

/// Unique ID of an operation.
#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    CandidType,
    Deserialize,
    Serialize,
    Hash,
)]
pub struct OperationId(u64);

impl OperationId {
    /// Creates new Id from the given number.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns a unique `nonce` value for given operation ID.
    pub fn nonce(&self) -> u32 {
        (self.0 % u32::MAX as u64) as u32
    }
}

impl Storable for OperationId {
    fn to_bytes(&self) -> Cow<[u8]> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(u64::from_bytes(bytes))
    }

    const BOUND: Bound = <u64 as Storable>::BOUND;
}

impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
