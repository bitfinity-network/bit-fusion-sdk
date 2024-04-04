mod build_data;

use candid::CandidType;
use ord_rs::OrdError;
use thiserror::Error;

pub use self::build_data::BuildData;

pub type InscribeResult<T> = Result<T, InscribeError>;

/// The InscribeTransactions struct is used to return the commit and reveal transactions.
#[derive(Debug, Clone, CandidType)]
pub struct InscribeTransactions {
    pub commit_tx: String,
    pub reveal_tx: String,
}

/// Error type for inscribe endpoint.
#[derive(Debug, Clone, CandidType, Error)]
pub enum InscribeError {
    #[error("bad address: {0}")]
    BadAddress(String),
    #[error("bad inscription: {0}")]
    BadInscription(String),
    #[error("inscribe error: {0}")]
    OrdError(String),
    #[error("failed to collect utxos: {0}")]
    FailedToCollectUtxos(String),
    #[error("not enough UTXOs allocated for fees: {0}")]
    InsufficientFundsForFees(String),
}

impl From<OrdError> for InscribeError {
    fn from(e: OrdError) -> Self {
        InscribeError::OrdError(e.to_string())
    }
}

#[derive(Debug, Clone, CandidType)]
pub struct InscriptionFees {
    pub commit_fee: u64,
    pub reveal_fee: u64,
    pub postage: u64,
}
