use bridge_did::order::SignedMintOrder;
use candid::CandidType;
use did::H256;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_exports::ic_kit::RejectionCode;
use ic_exports::icrc_types::icrc1::transfer::TransferError;
use serde::Deserialize;

use crate::ckbtc_client::{PendingUtxo, RetrieveBtcError, UpdateBalanceError};

/// Status of a pending BTC to ERC20 transfer.
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq)]
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
    /// manually to the BftBridge ot mint wrapped tokens.
    Signed(Box<SignedMintOrder>),
    /// Mint order for wrapped tokens is successfully sent to the BftBridge.
    Minted {
        /// Amount of tokens minted.
        amount: u64,
        /// EVM transaction ID.
        tx_id: H256,
    },
}

pub enum ErrorCodes {
    // Deposit errors
    ValueTooSmall = 0,
    Tainted = 1,
    CkBtcMinter = 2,
    CkBtcLedgerTransfer = 3,
    CkBtcLedgerBalance = 4,
    Sign = 5,
    NotInitialized = 6,
    NothingToMint = 7,
    WaitingForConfirmtions = 8,
    // Withdrawal errors
    InvalidRecipient = 9,
    RetrieveBtcError = 10,
}

/// Error during BTC to ERC20 transfer.
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Erc20MintError {
    /// The amount of BTC transferred to ckBTC is smaller than the fee. The transaction will not
    /// be precessed.
    ValueTooSmall,
    /// The BTC transferred to ckBTC did not pass the KYT check. The transaction will not be
    /// processed.
    Tainted(Utxo),
    /// Error while connecting to ckBTC.
    CkBtcMinter(UpdateBalanceError),
    /// Error transferring ckBTC tokens with ledger.
    CkBtcLedgerTransfer(TransferError),
    /// Error while checking the ledger balance.
    CkBtcLedgerBalance(RejectionCode, String),
    /// Error while signing the mint order.
    Sign(String),
    /// Error connecting to the EVM.
    Evm(String),
    /// BtcBridge canister is not properly initialized.
    NotInitialized,
    /// No pending transactions.
    NothingToMint,
    /// Waiting for confirmations on the UTXOs.
    WaitingForConfirmations,
}

impl From<TransferError> for Erc20MintError {
    fn from(value: TransferError) -> Self {
        Self::CkBtcLedgerTransfer(value)
    }
}

impl From<Erc20MintError> for bridge_did::error::Error {
    fn from(value: Erc20MintError) -> Self {
        match value {
            Erc20MintError::ValueTooSmall => Self::Custom {
                code: ErrorCodes::ValueTooSmall as u32,
                msg: "Value too small".to_string(),
            },
            Erc20MintError::Tainted(utxo) => Self::Custom {
                code: ErrorCodes::Tainted as u32,
                msg: format!("Tainted UTXO: {:?}", utxo),
            },
            Erc20MintError::CkBtcMinter(err) => Self::Custom {
                code: ErrorCodes::CkBtcMinter as u32,
                msg: format!("CkBtcMinter error: {:?}", err),
            },
            Erc20MintError::CkBtcLedgerTransfer(err) => Self::Custom {
                code: ErrorCodes::CkBtcLedgerTransfer as u32,
                msg: format!("CkBtcLedgerTransfer error: {:?}", err),
            },
            Erc20MintError::CkBtcLedgerBalance(code, msg) => Self::Custom {
                code: ErrorCodes::CkBtcLedgerBalance as u32,
                msg: format!("CkBtcLedgerBalance error: {msg} ({code:?})"),
            },
            Erc20MintError::Sign(msg) => Self::Custom {
                code: ErrorCodes::Sign as u32,
                msg,
            },
            Erc20MintError::NotInitialized => Self::Custom {
                code: ErrorCodes::NotInitialized as u32,
                msg: "Not initialized".to_string(),
            },
            Erc20MintError::NothingToMint => Self::Custom {
                code: ErrorCodes::NothingToMint as u32,
                msg: "Nothing to mint".to_string(),
            },
            Erc20MintError::WaitingForConfirmations => Self::Custom {
                code: ErrorCodes::WaitingForConfirmtions as u32,
                msg: "Waiting for confirmations".to_string(),
            },
            Erc20MintError::Evm(msg) => Self::EvmRequestFailed(msg),
        }
    }
}

/// Error during BTC withdrawal.
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum BtcWithdrawError {
    InvalidRecipient(Vec<u8>),
    RetrieveBtcError(String),
}

impl From<RetrieveBtcError> for BtcWithdrawError {
    fn from(err: RetrieveBtcError) -> Self {
        Self::RetrieveBtcError(match err {
            RetrieveBtcError::AlreadyProcessing => "Already processing".to_string(),
            RetrieveBtcError::AmountTooLow(amount) => format!("Amount too low: {}", amount),
            RetrieveBtcError::GenericError {
                error_message,
                error_code,
            } => {
                format!("Generic error: {} ({})", error_message, error_code)
            }
            RetrieveBtcError::InsufficientFunds { balance } => {
                format!("Insufficient funds: {}", balance)
            }
            RetrieveBtcError::MalformedAddress(address) => {
                format!("Malformed address: {}", address)
            }
            RetrieveBtcError::TemporarilyUnavailable(error_message) => {
                format!("Temporarily unavailable: {}", error_message)
            }
        })
    }
}

impl From<BtcWithdrawError> for bridge_did::error::Error {
    fn from(value: BtcWithdrawError) -> Self {
        match value {
            BtcWithdrawError::InvalidRecipient(recipient) => Self::Custom {
                code: ErrorCodes::InvalidRecipient as u32,
                msg: format!("Invalid recipient: {:?}", recipient),
            },
            BtcWithdrawError::RetrieveBtcError(msg) => Self::Custom {
                code: ErrorCodes::RetrieveBtcError as u32,
                msg,
            },
        }
    }
}
