use crate::utils;
use bitcoin::script::PushBytesBuf;
use ord_rs::{
    brc20::{Brc20, Brc20Deploy, Brc20Mint, Brc20Transfer},
    Inscription, OrdError, OrdResult,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Represents the type of digital artifact being inscribed.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Protocol {
    /// Satoshis imbued with `deploy`, `mint`, and `transfer` functionalities,
    /// as well as token supply, simulating fungibility (e.g., like ERC20 tokens).
    Brc20 { func: Brc20Func },
    /// For now, we refer to all other inscriptions (i.e. non-BRC20 ords) as
    /// non-fungible (e.g., like ERC721 tokens).
    Nft,
}

/// Represents a BRC20 operation/function
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Brc20Func {
    Deploy,
    Mint,
    Transfer,
}

/// Represents an arbitrary Ordinal inscription with optional metadata and content.
///
/// For now, we refer to this as an NFT (e.g., like an ERC721 token).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Nft {
    /// The main body of the inscription. This could be the actual data or content
    /// inscribed onto a Bitcoin satoshi.
    pub body: Option<String>,
    /// Specifies the MIME type of the `body` content, such as `text/plain` for text,
    /// `image/png` for images, etc., to inform how the data should be interpreted.
    pub content_type: Option<String>,
    /// Optional metadata associated with the inscription. This could be used to store
    /// additional information about the inscription, such as creator identifiers, timestamps,
    /// or related resources.
    pub metadata: Option<String>,
}

impl Nft {
    /// Creates a new `Nft` with optional data.
    pub fn new(
        content_type: Option<String>,
        body: Option<String>,
        metadata: Option<String>,
    ) -> Self {
        Self {
            content_type,
            body,
            metadata,
        }
    }

    /// Encode Self as a JSON string
    fn encode(&self) -> OrdResult<String> {
        Ok(serde_json::to_string(self)?)
    }
}

impl FromStr for Nft {
    type Err = OrdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(OrdError::from)
    }
}

impl Inscription for Nft {
    fn content_type(&self) -> String {
        self.content_type
            .as_ref()
            .map(|c| c.to_string())
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

pub enum InscriptionType {
    Brc20 { inner: Brc20 },
    Nft { inner: Nft },
}

pub fn handle_inscriptions(protocol: Protocol, data: &str) -> OrdResult<InscriptionType> {
    match protocol {
        Protocol::Brc20 { func } => match func {
            Brc20Func::Deploy => {
                let deploy = serde_json::from_str::<Brc20Deploy>(data)?;
                let deploy_op = Brc20::deploy(deploy.tick, deploy.max, deploy.lim, deploy.dec);
                Ok(InscriptionType::Brc20 { inner: deploy_op })
            }
            Brc20Func::Mint => {
                let mint = serde_json::from_str::<Brc20Mint>(data)?;
                let mint_op = Brc20::mint(mint.tick, mint.amt);
                Ok(InscriptionType::Brc20 { inner: mint_op })
            }
            Brc20Func::Transfer => {
                let transfer = serde_json::from_str::<Brc20Transfer>(data)?;
                let transfer_op = Brc20::transfer(transfer.tick, transfer.amt);
                Ok(InscriptionType::Brc20 { inner: transfer_op })
            }
        },
        Protocol::Nft => {
            let nft_data = serde_json::from_str::<Nft>(data)?;
            let nft = Nft::new(nft_data.content_type, nft_data.body, nft_data.metadata);
            Ok(InscriptionType::Nft { inner: nft })
        }
    }
}
