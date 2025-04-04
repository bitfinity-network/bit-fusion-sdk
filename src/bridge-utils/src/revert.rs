use std::string::FromUtf8Error;

use hex::FromHexError;

/// Error marker to find the revert reason in the data.
pub const ERROR_MARKER: &str = "0x08c379a0"; // revert(string) signature

/// Error type for parsing revert reasons.
#[derive(Debug, thiserror::Error)]
pub enum ParseRevertError {
    #[error("invalid format")]
    InvalidFormat,
    #[error("failed to decode revert reason from hex: {0}")]
    DecodeError(#[from] FromHexError),
    #[error("failed to parse u64 from slice: {0}")]
    SliceError(#[from] std::array::TryFromSliceError),
    #[error("failed to parse revert reason: {0}")]
    Utf8Error(#[from] FromUtf8Error),
    #[error("bad length: {actual} is less than expected {expected}")]
    LengthError { expected: usize, actual: usize },
}

/// Parses the revert reason from the given data.
pub fn parse_revert_reason(data: &str) -> Result<String, ParseRevertError> {
    const DATA_ZERO: usize = 4;
    const OFFSET_START: usize = DATA_ZERO + 24;
    const OFFSET_END: usize = OFFSET_START + 8;
    const LENGTH_START: usize = OFFSET_START + 32;
    const LENGTH_END: usize = LENGTH_START + 8;

    // Check if the data starts with the error marker
    if !data.starts_with(ERROR_MARKER) {
        return Err(ParseRevertError::InvalidFormat);
    }

    // decode to bytes
    let data = hex::decode(data.trim_start_matches("0x")).map_err(ParseRevertError::from)?;

    // data length must be at least LENGTH_END
    if data.len() < LENGTH_END {
        return Err(ParseRevertError::LengthError {
            expected: LENGTH_END,
            actual: data.len(),
        });
    }

    // read the string offset
    let offset = u64::from_be_bytes(
        data[OFFSET_START..OFFSET_END]
            .try_into()
            .map_err(ParseRevertError::from)?,
    );

    // read the string length
    let length = u64::from_be_bytes(
        data[LENGTH_START..LENGTH_END]
            .try_into()
            .map_err(ParseRevertError::from)?,
    );
    // read the string
    let start = OFFSET_END + offset as usize;
    let end = start + length as usize;

    // check if the string is in bounds
    if end > data.len() {
        return Err(ParseRevertError::LengthError {
            expected: end,
            actual: data.len(),
        });
    }

    let reason = &data[start..end];
    // convert to utf8
    String::from_utf8(reason.to_vec()).map_err(ParseRevertError::from)
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_parse_revert_reason() {
        let data = "0x08c379a000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000012496e76616c696420746f6b656e20706169720000000000000000000000000000";

        let result = parse_revert_reason(data);
        assert!(result.is_ok());
        let reason = result.unwrap();
        assert_eq!(reason, "Invalid token pair");
    }
}
