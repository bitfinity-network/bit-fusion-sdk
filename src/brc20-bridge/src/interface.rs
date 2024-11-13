use bridge_did::order::SignedMintOrder;
use candid::CandidType;
use did::H256;
use serde::Deserialize;
use thiserror::Error;

use crate::core::deposit::Brc20DepositPayload;
use crate::key::KeyError;

#[derive(Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub struct PendingUtxo {}

/// Status of a pending BTC to ERC20 transfer.
#[derive(Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum Erc20MintStatus {
    /// The BTC transfer is found, but it doesn't have enough confirmations yet. After enough
    /// confirmations are received, the transaction will be precessed automatically, no additional
    /// actions are required from the user.
    Scheduled {
        /// Current confirmations of the transaction.
        current_confirmations: u32,
        /// Number of confirmations required by ckBTC minter canister to mint ckBTC tokens.
        required_confirmations: u32,
        /// Pending transactions.
        pending_utxos: Option<Vec<PendingUtxo>>,
    },
    /// The transaction is processed, ckBTC tokens are minted and mint order is created. But there
    /// was a problem sending the mint order to the EVM. The given signed mint order can be sent
    /// manually to the Btfbridge ot mint wrapped tokens.
    Signed(Box<SignedMintOrder>),
    /// Mint order for wrapped tokens is successfully sent to the Btfbridge.
    Minted {
        /// Amount of tokens minted.
        amount: u128,
        /// EVM transaction ID.
        tx_id: H256,
    },
}

/// Error during BTC to ERC20 transfer.
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Erc20MintError {
    /// The amount of BTC transferred to ckBTC is smaller than the fee. The transaction will not
    /// be precessed.
    ValueTooSmall,
    /// Error while signing the mint order.
    Sign(String),
    /// Error connecting to the EVM.
    Evm(String),
    /// BtcBridge canister is not properly initialized.
    NotInitialized,
    /// No pending transactions.
    NothingToMint,
}

#[derive(Debug, Error, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum GetAddressError {
    #[error("key error: {0}")]
    Key(String),
}

impl From<KeyError> for GetAddressError {
    fn from(e: KeyError) -> Self {
        Self::Key(e.to_string())
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct DepositResponse {
    pub mint_order_result: Erc20MintStatus,
}

#[derive(Debug, Error, Clone, CandidType, Deserialize)]
pub enum DepositError {
    #[error("not initialized")]
    NotInitialized,
    #[error("signer not initialized")]
    SignerNotInitialized,
    #[error("not scheduled")]
    NotScheduled,
    #[error("nothing to deposit")]
    NothingToDeposit,
    #[error("no BRC20 to deposit")]
    NoBrc20ToDeposit,
    #[error("deposit UTXOs are not confirmed")]
    UtxosNotConfirmed,
    #[error("no destination token address")]
    NoDstTokenAddress,
    #[error("the provided UTXOs are already used")]
    UtxoAlreadyUsed,
    #[error("the amount is too big: {0}")]
    AmountTooBig(String),
    #[error("key error: {0}")]
    KeyError(String),
    #[error("indexers disagree: {indexer_responses:?}")]
    IndexersDisagree {
        indexer_responses: Vec<(String, String)>,
    },
    #[error("insufficient consensus: received {received_responses}/{required_responses}, checked {checked_indexers}")]
    InsufficientConsensus {
        received_responses: usize,
        required_responses: u8,
        checked_indexers: usize,
    },
    #[error("invalid amounts: requested {requested}, actual {actual}")]
    InvalidAmounts { requested: u128, actual: u128 },
    #[error("not enough BTC: received {received}, minimum {minimum}")]
    NotEnoughBtc { received: u64, minimum: u64 },
    #[error("unavailable: {0}")]
    Unavailable(String),
    #[error("pending; min confirmations: {min_confirmations}, current confirmations: {current_confirmations}")]
    Pending {
        min_confirmations: u32,
        current_confirmations: u32,
    },
    #[error("sign error: {0}")]
    /// Error while signing the mint order.
    Sign(String),
    #[error("EVM error: {0}")]
    Evm(String),
}

impl From<KeyError> for DepositError {
    fn from(e: KeyError) -> Self {
        Self::KeyError(e.to_string())
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum WithdrawError {
    AmountTooBig(u128),
    NoInputs,
    TxNotConfirmed,
    InvalidTxid(Vec<u8>),
    TransactionCreation,
    TransactionSigning(String),
    TransactionSerialization,
    TransactionSending,
    FeeRateRequest,
    InsufficientFunds,
    SignerNotInitialized,
    FailedToGetPubkey(String),
    FailedToGenerateTaprootKeypair,
    CommitTransactionError(String),
    RevealTransactionError(String),
    KeyError(String),
    InvalidRequest(String),
    InternalError(String),
}

impl From<KeyError> for WithdrawError {
    fn from(e: KeyError) -> Self {
        Self::KeyError(e.to_string())
    }
}

#[derive(Debug, Copy, Clone, CandidType, Deserialize, Hash, PartialEq, Eq)]
pub struct RuneIdDid {
    pub block_id: u64,
    pub txid: u32,
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct DepositStateResponse {
    pub current_ts: u64,
    pub deposits: Vec<Brc20DepositPayload>,
}
