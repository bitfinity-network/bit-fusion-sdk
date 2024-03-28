mod build_data;

use candid::CandidType;
use ord_rs::OrdError;
use thiserror::Error;

pub use self::build_data::BuildData;

pub type InscribeResult<T> = Result<T, InscribeError>;

#[derive(Debug, Clone, CandidType)]
/// The InscribeTransactions struct is used to return the commit and reveal transactions.
pub struct InscribeTransactions {
    pub commit_tx: String,
    pub reveal_tx: String,
}

#[derive(Debug, Clone, CandidType, Error)]
/// Error type for inscribe endpoint.
pub enum InscribeError {
    #[error("bad address: {0}")]
    BadAddress(String),
    #[error("bad inscription: {0}")]
    BadInscription(String),
    #[error("inscribe error: {0}")]
    OrdError(String),
    #[error("failed to collect utxos: {0}")]
    FailedToCollectUtxos(String),
}

impl From<OrdError> for InscribeError {
    fn from(e: OrdError) -> Self {
        InscribeError::OrdError(e.to_string())
    }
}
