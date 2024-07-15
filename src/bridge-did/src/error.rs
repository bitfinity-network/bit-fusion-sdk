use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::op_id::OperationId;

pub type BftResult<T> = Result<T, Error>;

#[derive(Debug, Error, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum Error {
    #[error("the caller have no permission to perform the action")]
    AccessDenied,

    #[error("initialization failure: {0}")]
    Initialization(String),

    #[error("serializer failure: {0}")]
    Serialization(String),

    #[error("signer failure: {0}")]
    Signing(String),

    #[error("operation#{0} not found")]
    OperationNotFound(OperationId),

    #[error("unexpected anonymous principal")]
    AnonymousPrincipal,

    #[error("generic error: code=={code}, message=`{msg}`")]
    Custom { code: u32, msg: String },
}
