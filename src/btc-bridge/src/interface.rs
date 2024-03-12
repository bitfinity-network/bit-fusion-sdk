use crate::ck_btc_interface::{PendingUtxo, UpdateBalanceError};
use candid::CandidType;
use did::H256;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use minter_did::order::SignedMintOrder;
use serde::Deserialize;

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

/// Error during BTC to ERC20 transfer.
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Erc20MintError {
    /// The amount of BTC transferred to ckBTC is smaller than the fee. The transaction will not
    /// be precessed.
    ValueTooSmall(Utxo),
    /// The BTC transferred to ckBTC did not pass the KYT check. The transaction will not be
    /// processed.
    Tainted(Utxo),
    /// Error while connecting to ckBTC.
    CkBtcError(UpdateBalanceError),
    /// Error while signing the mint order.
    Sign(String),
    /// Error connecting to the EVM.
    Evm(String),
    /// BtcBridge canister is not properly initialized.
    NotInitialized,
    /// No pending transactions.
    NothingToMint,
}