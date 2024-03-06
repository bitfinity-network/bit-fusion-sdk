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

/// Arguments for creating a commit transaction
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct CommitTransactionArgs {
    /// Inputs of the transaction
    pub inputs: Vec<TxInput>,
    /// Inscription to write
    pub inscription: String,
    /// Address to send the leftovers BTC of the trasnsaction
    pub leftovers_recipient: String,
    /// Fee to pay for the commit transaction
    pub commit_fee: u64,
    /// Fee to pay for the reveal transaction
    pub reveal_fee: u64,
    /// Script pubkey of the inputs
    pub txin_script_pubkey: String,
}

/// Commit transaction input type
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct TxInput {
    pub id: String,
    pub index: u32,
    pub amount: u64,
}
