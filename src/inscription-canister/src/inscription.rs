use crate::utils;
use bitcoin::script::PushBytesBuf;
use ord_rs::{brc20::Brc20, Inscription, OrdResult};
use serde::{Deserialize, Serialize};

/// Represents the type of digital artifact being inscribed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Protocol {
    /// Satoshis imbued with `deploy`, `mint`, and `transfer` functionalities,
    /// as well as token supply, simulating fungibility (e.g., like ERC20 tokens).
    Brc20(Brc20),
    /// For now, we refer to all other inscriptions (i.e. non-BRC20 ords) as
    /// non-fungible (e.g., like ERC721 tokens).
    Nft(Nft),
}

impl Inscription for Protocol {
    fn content_type(&self) -> String {
        match self {
            Self::Brc20(brc20) => brc20.content_type(),
            Self::Nft(nft) => nft.content_type(),
        }
    }

    fn data(&self) -> OrdResult<PushBytesBuf> {
        match self {
            Self::Brc20(brc20) => brc20.data(),
            Self::Nft(nft) => nft.data(),
        }
    }

    fn parse(data: &[u8]) -> OrdResult<Self>
    where
        Self: Sized,
    {
        Ok(serde_json::from_str(&String::from_utf8_lossy(data))?)
    }
}

/// Represents an arbitrary Ordinal inscription with optional metadata and content.
///
/// For now, we refer to this as an NFT (e.g., like an ERC721 token).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Nft {
    /// The main body of the inscription. This could be the actual data or content
    /// inscribed onto a Bitcoin satoshi.
    pub body: Option<Vec<u8>>,
    /// Specifies the MIME type of the `body` content, such as `text/plain` for text,
    /// `image/png` for images, etc., to inform how the data should be interpreted.
    pub content_type: Option<Vec<u8>>,
    /// Optional metadata associated with the inscription. This could be used to store
    /// additional information about the inscription, such as creator identifiers, timestamps,
    /// or related resources.
    pub metadata: Option<Vec<u8>>,
}

impl Nft {
    /// Creates a new `Nft` with no metadata.
    pub fn new(body: Option<Vec<u8>>, content_type: Option<Vec<u8>>) -> Self {
        Self {
            body,
            content_type,
            ..Default::default()
        }
    }

    /// Creates a new `Nft` with some metadata.
    pub fn new_with_metadata(
        body: Option<Vec<u8>>,
        content_type: Option<Vec<u8>>,
        metadata: Option<Vec<u8>>,
    ) -> Self {
        Self {
            body,
            content_type,
            metadata,
        }
    }

    /// Encode Self as a JSON string
    fn encode(&self) -> OrdResult<String> {
        Ok(serde_json::to_string(self)?)
    }
}

impl Inscription for Nft {
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
