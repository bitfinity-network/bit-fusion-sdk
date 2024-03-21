use candid::{CandidType, Deserialize};
use ic_canister_client::CanisterClientError;
use ord_rs::OrdError;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TestError {
    #[error(transparent)]
    Ord(#[from] OrdError),

    #[error(transparent)]
    Candid(#[from] candid::Error),

    #[error(transparent)]
    CanisterClient(#[from] CanisterClientError),

    #[error("{0}")]
    Generic(String),
}

#[derive(Error, Debug, CandidType, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum InscriberCallError {
    #[error("failed to update Inscriber canister")]
    InscriberUpdate,
    #[error("unauthorized principal")]
    Unauthorized,
}

pub type TestResult<T> = std::result::Result<T, TestError>;
pub type InscriberCallResult<T> = std::result::Result<T, InscriberCallError>;
