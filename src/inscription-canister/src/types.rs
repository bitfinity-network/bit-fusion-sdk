use candid::{CandidType, Deserialize};

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct CommitTransactionArgs {
    pub inputs: Vec<TxInput>,
    pub inscription: Inscription,
    pub leftovers_recipient: String,
    pub commit_fee: u64,
    pub reveal_fee: u64,
    pub txin_script_pubkey: Vec<u8>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Inscription {
    pub body: Option<Vec<u8>>,
    pub content_encoding: Option<Vec<u8>>,
    pub content_type: Option<Vec<u8>>,
    pub metadata: Option<Vec<u8>>,
    pub unrecognized_even_field: bool,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct CommitTransactionReply {
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

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct RevealTransactionArgs {
    pub input: TxInput,
    pub recipient_address: String,
    pub redeem_script: Vec<u8>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TxInput {
    pub id: Txid,
    pub index: u32,
    pub amount: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
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
    pub txid: Txid,
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
