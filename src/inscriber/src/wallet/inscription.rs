use candid::{CandidType, Deserialize};
use serde::Serialize;

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

/// Non-BRC20 (arbitrary) inscriptions
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct Nft {
    /// The main body of the NFT.
    pub body: Option<String>,
    /// Has a tag of 1, representing the MIME type of the body.
    pub content_type: Option<String>,
    /// Has a tag of 5, representing CBOR metadata, stored as data pushes.
    pub metadata: Option<String>,
}
