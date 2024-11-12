use std::collections::HashMap;

use bridge_did::order::SignedMintOrder;
use bridge_did::runes::RuneName;
use candid::CandidType;
use did::H256;
use ordinals::Pile;
use serde::Deserialize;

use crate::core::deposit::RuneDepositPayload;
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

#[derive(Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum GetAddressError {
    Key(String),
}

impl From<KeyError> for GetAddressError {
    fn from(e: KeyError) -> Self {
        GetAddressError::Key(e.to_string())
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct DepositResponse {
    pub mint_order_result: Erc20MintStatus,
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum DepositError {
    NotInitialized,
    NotScheduled,
    NothingToDeposit,
    NoRunesToDeposit,
    UtxosNotConfirmed,
    NoDstTokenAddress,
    UtxoAlreadyUsed,
    SignerNotInitialized,
    KeyError(String),
    InvalidAmounts {
        requested: HashMap<RuneName, u128>,
        actual: HashMap<RuneName, u128>,
    },
    NotEnoughBtc {
        received: u64,
        minimum: u64,
    },
    Unavailable(String),
    Pending {
        min_confirmations: u32,
        current_confirmations: u32,
    },
    /// Error while signing the mint order.
    Sign(String),
    Evm(String),
    Other(String),
    NoConsensus {
        first_response: String,
        another_response: String,
    },
}

impl From<KeyError> for DepositError {
    fn from(e: KeyError) -> Self {
        DepositError::KeyError(e.to_string())
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum WithdrawError {
    SignerNotInitialized,
    NoInputs,
    TransactionCreation,
    TransactionSigning,
    TransactionSerialization,
    TransactionSending,
    FeeRateRequest,
    ChangeAddress,
    InsufficientFunds,
    InvalidRequest(String),
    InternalError(String),
    KeyError(String),
}

impl From<KeyError> for WithdrawError {
    fn from(e: KeyError) -> Self {
        WithdrawError::KeyError(e.to_string())
    }
}

#[derive(Debug, Copy, Clone, CandidType, Deserialize, Hash, PartialEq, Eq)]
pub struct RuneIdDid {
    pub block_id: u64,
    pub txid: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OutputResponse {
    pub address: String,
    #[serde(default)]
    pub runes: HashMap<String, Pile>,
    pub spent: bool,
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct DepositStateResponse {
    pub current_ts: u64,
    pub deposits: Vec<RuneDepositPayload>,
}
