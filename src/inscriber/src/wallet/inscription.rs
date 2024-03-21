use candid::{CandidType, Deserialize};
use serde::Serialize;

/// Type of digital artifact being inscribed.
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub enum Protocol {
    Brc20,
    Nft,
}

/// BRC-20 operations
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub enum Brc20 {
    /// Deploy a BRC-20 token
    Deploy(Brc20Deploy),
    /// Mint BRC-20 tokens
    Mint(Brc20Mint),
    /// Transfer BRC-20 tokens
    Transfer(Brc20Transfer),
}

/// BRC-20 deploy operation
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct Brc20Deploy {
    pub tick: String,
    pub max: u64,
    pub lim: Option<u64>,
    pub dec: Option<u64>,
}

/// BRC-20 mint operation
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct Brc20Mint {
    pub tick: String,
    pub amt: u64,
}

/// BRC-20 transfer operation
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct Brc20Transfer {
    pub tick: String,
    pub amt: u64,
}

#[derive(CandidType, Clone, Debug, Serialize, Default, Deserialize)]
pub struct Nft {
    /// The MIME type of the body. This describes
    /// the format of the body content, such as "image/png" or "text/plain".
    pub content_type: String,
    /// The main body of the NFT. This is the core data or content of the NFT,
    /// which might represent an image, text, or other types of digital assets.
    pub body: String,
}
