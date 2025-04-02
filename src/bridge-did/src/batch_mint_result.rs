use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result of a batch mint operation on the BFT bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
#[repr(u8)]
pub enum BatchMintErrorCode {
    Ok = 0,
    InsufficientFeeDeposit = 1,
    ZeroAmount = 2,
    UsedNonce = 3,
    ZeroRecipient = 4,
    UnexpectedRecipientChainId = 5,
    TokensNotBridged = 6,
    ProcessingNotRequested = 7,
}

impl BatchMintErrorCode {
    /// Returns whether the batch mint operation was successful, i.e., if the result is [`BatchMintResult::Ok`].
    pub fn is_ok(&self) -> bool {
        matches!(self, BatchMintErrorCode::Ok)
    }
}

impl TryFrom<u8> for BatchMintErrorCode {
    type Error = BatchMintResultError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(BatchMintErrorCode::Ok),
            1 => Ok(BatchMintErrorCode::InsufficientFeeDeposit),
            2 => Ok(BatchMintErrorCode::ZeroAmount),
            3 => Ok(BatchMintErrorCode::UsedNonce),
            4 => Ok(BatchMintErrorCode::ZeroRecipient),
            5 => Ok(BatchMintErrorCode::UnexpectedRecipientChainId),
            6 => Ok(BatchMintErrorCode::TokensNotBridged),
            7 => Ok(BatchMintErrorCode::ProcessingNotRequested),
            value => Err(BatchMintResultError::UnknownError(value)),
        }
    }
}

/// Error codes for batch mint result.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum BatchMintResultError {
    #[error("parse error: {0}")]
    Parse(#[from] alloy_sol_types::Error),
    #[error("unknown error: {0}")]
    UnknownError(u8),
}
