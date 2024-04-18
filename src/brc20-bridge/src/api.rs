use candid::CandidType;
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Outpoint;
use minter_did::order::SignedMintOrder;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::inscriber_api::{InscribeTransactions, Multisig, Protocol};

#[derive(Error, CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum BridgeError {
    #[error("{0}")]
    InscriptionParsing(String),
    #[error("{0}")]
    InscriptionValidation(String),
    #[error("{0}")]
    GetDepositAddress(String),
    #[error("{0}")]
    FetchBrc20TokenDetails(String),
    #[error("{0}")]
    GetTransactionById(String),
    #[error("{0}")]
    PublicKeyFromStr(String),
    #[error("{0}")]
    AddressFromPublicKey(String),
    #[error("{0}")]
    EcdsaPublicKey(String),
    #[error("{0}")]
    SetTokenSymbol(String),
    #[error("{0}")]
    Brc20Burn(String),
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct Brc20TokenDetails {
    pub ticker: String,
    pub holder: String,
    pub tx_id: String,
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct InscribeBrc20Args {
    pub inscription_type: Protocol,
    pub inscription: String,
    pub leftovers_address: String,
    pub dst_address: String,
    pub multisig_config: Option<Multisig>,
}

/// Arguments to `Brc20Task::MintErc20`
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MintErc20Args {
    /// User's ETH address
    pub address: H160,
    /// ID of the reveal transaction
    pub reveal_txid: String,
}

/// Status of an ERC20 to a BRC20 swap
#[derive(CandidType, Clone, Debug, Deserialize)]
pub struct Brc20InscribeStatus {
    /// commit_txid and reveal_txid
    pub tx_ids: InscribeTransactions,
}

/// Errors that occur during an ERC20 to a BRC20 swap.
#[derive(Error, CandidType, Clone, Debug, Deserialize)]
pub enum Brc20InscribeError {
    /// Error from the Inscriber regarding a BRC20 transfer call
    #[error("{0}")]
    Brc20Transfer(String),
    /// Error returned by the `inscribe` endpoint of the Inscriber.
    #[error("{0}")]
    Inscribe(String),
    /// The bitcoin address is invalid.
    #[error("{0}")]
    MalformedAddress(String),
    /// There are too many concurrent requests, retry later.
    #[error("{0}")]
    TemporarilyUnavailable(String),
}

#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct PendingUtxo {
    pub outpoint: Outpoint,
    pub value: u64,
    pub confirmations: u32,
}

/// Status of a BRC20 to ERC20 swap
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Erc20MintStatus {
    /// The BTC transfer is found, but it doesn't have enough confirmations yet. After enough
    /// confirmations are received, the transaction will be precessed automatically, no additional
    /// actions are required from the user.
    Scheduled {
        /// Current number of confirmations for the transaction.
        current_confirmations: u32,
        /// Number of confirmations required by the inscriber canister to create the BRC20.
        required_confirmations: u32,
        /// Pending transactions.
        pending_utxos: Option<Vec<PendingUtxo>>,
    },
    /// This happens when the transaction is processed, the BRC20 inscription is parsed and validated,
    /// and the mint order is created; however, there is a problem sending the mint order to the EVM.
    /// The signed mint order can be sent manually to the BftBridge to mint wrapped tokens.
    Signed(Box<SignedMintOrder>),
    /// Mint order for wrapped tokens is successfully sent to the `BftBridge`.
    Minted {
        /// Amount of tokens minted.
        amount: u64,
        /// EVM transaction ID.
        tx_id: H256,
    },
}

/// Errors that occur during a BRC20 to ERC20 swap.
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Erc20MintError {
    /// Error from the Brc20Bridge
    Brc20Bridge(String),
    /// The Brc20Bridge is not properly initialized.
    NotInitialized,
    /// Error while connecting to the Inscriber.
    Inscriber(String),
    /// Error connecting to the EVM.
    Evm(String),
    /// The inscription (BRC20) received is invalid.
    InvalidBrc20(String),
    /// The BRC20's amount is smaller than the fee. The transaction will not
    /// be precessed.
    ValueTooSmall,
    /// Error while signing the mint order.
    Sign(String),
}
