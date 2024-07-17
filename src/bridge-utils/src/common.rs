use candid::CandidType;
use serde::Deserialize;

/// Fetching pagination parameters.
#[derive(Debug, Deserialize, CandidType)]
pub struct Pagination {
    /// The number of items to skip.
    pub offset: usize,
    /// The number of items to return.
    pub count: usize,
}

impl Pagination {
    /// Create a new pagination.
    pub fn new(offset: usize, count: usize) -> Self {
        Self { offset, count }
    }
}

impl From<(usize, usize)> for Pagination {
    fn from((offset, count): (usize, usize)) -> Self {
        Self::new(offset, count)
    }
}
