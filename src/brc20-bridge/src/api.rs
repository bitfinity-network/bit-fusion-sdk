use candid::CandidType;
use did::H256;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Outpoint;
use minter_did::order::SignedMintOrder;
use serde::Deserialize;

#[derive(thiserror::Error, CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum BridgeError {
    #[error("{0}")]
    GetDepositAddress(String),
    #[error("{0}")]
    GetUtxos(String),
    #[error("{0}")]
    GetBalance(String),
    #[error("{0}")]
    GetTransactionById(String),
    #[error("{0}")]
    PublicKeyFromStr(String),
    #[error("{0}")]
    AddressFromPublicKey(String),
    #[error("{0}")]
    EcdsaPublicKey(String),
}

/// Status of an ERC20 to a BRC20 swap
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Brc20InscribeStatus {
    /// Index of the inscription block
    pub inscription_index: u64,
}

/// Errors that occur during an ERC20 to a BRC20 swap.
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum Brc20InscribeError {
    /// There is another request for this principal.
    InscriberBusy,

    /// The amount specified for the inscription is too low.
    LowPostage(u64),

    /// The bitcoin address is invalid.
    MalformedAddress(String),

    /// The withdrawal account does not hold the requested amount.
    InsufficientFunds { balance: u64 },

    /// There are too many concurrent requests, retry later.
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
    /// The Brc20Bridge is not properly initialized.
    NotInitialized,
    /// Error while connecting to the Inscriber.
    Inscriber(String),
    /// Error connecting to the EVM.
    Evm(String),
    /// The inscription (BRC20) received is invalid.
    InvalidBrc20,
    /// The BRC20's amount is smaller than the fee. The transaction will not
    /// be precessed.
    ValueTooSmall,
    /// Error while signing the mint order.
    Sign(String),
    /// No pending mint transactions.
    MintQueueEmpty,
}
