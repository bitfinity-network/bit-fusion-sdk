//! Utility functions and types

use candid::{CandidType, Deserialize};

#[derive(CandidType, Deserialize, Debug)]
pub struct CreateCommitTransactionArgs {
    pub inputs: Vec<TxInput>,
    pub inscription: String,
    pub leftovers_recipient: String,
    pub commit_fee: u64,
    pub reveal_fee: u64,
    pub txin_script_pubkey: String,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct CreateCommitTransaction {
    pub tx: Transaction,
    pub redeem_script: Vec<u8>,
    pub reveal_balance: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Transaction {
    pub version: i32,
    pub lock_time: LockTime,
    pub input: Vec<TxIn>,
    pub output: Vec<TxOut>,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct RevealTransactionArgs {
    pub input: TxInput,
    pub recipient_address: String,
    pub redeem_script: Vec<u8>,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct TxInput {
    pub id: String,
    pub index: u32,
    pub amount: u64,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct Txid {
    tx_id: String,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TxIn {
    pub previous_output: OutPoint,
    pub script_sig: Vec<u8>,
    pub sequence: u32,
    pub witness: Witness,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct OutPoint {
    pub txid: String,
    pub vout: u32,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Witness {
    content: Vec<u8>,
    witness_elements: usize,
    indices_start: usize,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TxOut {
    pub value: u64,
    pub script_pubkey: Vec<u8>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum LockTime {
    Blocks(u32),
    Seconds(u32),
}
