use crate::utils;
use bitcoin::script::PushBytesBuf;
use ord_rs::{Inscription, OrdError, OrdResult};
use serde::{Deserialize, Serialize};

/// Represents an Ordinal inscription with optional metadata and content.
///
/// This struct encapsulates the data and metadata associated with an Ordinal inscription,
/// providing flexibility for various types of content and encoding formats.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct OrdInscription {
    /// The main body of the inscription. This could be the actual data or content
    /// inscribed onto a Bitcoin satoshi.
    pub body: Option<Vec<u8>>,

    /// Specifies the encoding of the `body` content. Examples might include `UTF-8`
    /// for text, or other encoding types for binary data.
    pub content_encoding: Option<Vec<u8>>,

    /// Specifies the MIME type of the `body` content, such as `text/plain` for text,
    /// `image/png` for images, etc., to inform how the data should be interpreted.
    pub content_type: Option<Vec<u8>>,

    /// Optional metadata associated with the inscription. This could be used to store
    /// additional information about the inscription, such as creator identifiers, timestamps,
    /// or related resources.
    pub metadata: Option<Vec<u8>>,

    /// Indicates whether the inscription data includes fields that were not recognized.
    /// This can be useful for forward compatibility, allowing newer versions of software
    /// to identify data that they do not understand.
    pub unrecognized_even_field: bool,
}

impl OrdInscription {
    /// Creates a new `OrdInscription` instance with default values.
    pub fn new(
        body: Option<Vec<u8>>,
        content_encoding: Option<Vec<u8>>,
        content_type: Option<Vec<u8>>,
        metadata: Option<Vec<u8>>,
        unrecognized_even_field: bool,
    ) -> Self {
        Self {
            body,
            content_encoding,
            content_type,
            metadata,
            unrecognized_even_field,
        }
    }

    /// Parses a serialized `OrdInscription` from a slice of bytes.
    ///
    /// This associated function attempts to deserialize an `OrdInscription` from a given
    /// byte slice, typically representing serialized data retrieved from a storage medium
    /// or a network source.
    pub fn from_bytes(data: &[u8]) -> OrdResult<Self> {
        serde_json::from_slice(data).map_err(OrdError::Codec)
    }

    /// Serializes the `OrdInscription` into a vector of bytes.
    ///
    /// This method converts the `OrdInscription` into a byte vector, suitable for storage
    /// or transmission. The serialization format used is determined by the implementation
    /// (e.g., JSON, binary, etc.).
    pub fn to_bytes(&self) -> OrdResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(OrdError::Codec)
    }

    pub fn body(&self) -> Option<&[u8]> {
        Some(self.body.as_ref()?)
    }

    pub fn content_type(&self) -> Option<&str> {
        std::str::from_utf8(self.content_type.as_ref()?).ok()
    }

    /// Encode Self as a JSON string
    fn encode(&self) -> OrdResult<String> {
        Ok(serde_json::to_string(self)?)
    }
}

impl Inscription for OrdInscription {
    fn content_type(&self) -> String {
        self.content_type
            .as_ref()
            .map(|bytes| String::from_utf8_lossy(bytes).to_string())
            .unwrap_or_default()
    }

    fn data(&self) -> OrdResult<PushBytesBuf> {
        utils::to_push_bytes(self.encode()?.as_bytes())
    }

    fn parse(data: &[u8]) -> OrdResult<Self>
    where
        Self: Sized,
    {
        Ok(serde_json::from_str(&String::from_utf8_lossy(data))?)
    }
}
