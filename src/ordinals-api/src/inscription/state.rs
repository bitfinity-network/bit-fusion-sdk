use candid::CandidType;
use serde::{Deserialize, Serialize};

/// Inscription response api schema.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct Inscription {
    pub id: String,
    pub number: u64,
    pub address: Option<String>,
    pub genesis_address: Option<String>,
    pub genesis_block_height: u64,
    pub genesis_block_hash: String,
    pub genesis_tx_id: String,
    pub genesis_fee: String,
    pub genesis_timestamp: i64,
    pub tx_id: String,
    pub location: String,
    pub output: String,
    pub value: Option<String>,
    pub offset: Option<String>,
    pub sat_ordinal: String,
    pub sat_rarity: String,
    pub sat_coinbase_height: u64,
    pub mime_type: String,
    pub content_type: String,
    pub content_length: u64,
    pub timestamp: i64,
    pub curse_type: Option<String>,
    pub recursive: bool,
    pub recursion_refs: Option<Vec<String>>,
}

/// Inscription location response api schema.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct InscriptionLocation {
    pub block_height: u64,
    pub block_hash: String,
    pub address: Option<String>,
    pub tx_id: String,
    pub location: String,
    pub output: String,
    pub value: Option<String>,
    pub offset: Option<String>,
    pub timestamp: i64,
}
