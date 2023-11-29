use thiserror::Error;

#[derive(Debug, Error)]
pub enum SolidityHelperError {
    #[error("GenericError: {0:?}")]
    GenericError(String),

    #[error("FromHexError: {0:?}")]
    FromHexError(#[from] hex::FromHexError),

    #[error("IoError: {0:?}")]
    IoError(#[from] std::io::Error),

    #[error("SerdeJsonError: {0:?}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("JsonFieldNotFoundError: {0:?}")]
    JsonFieldNotFoundError(&'static str),
}
