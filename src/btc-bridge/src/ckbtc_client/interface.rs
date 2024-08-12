//! The types in this module are copied from IC ckBTC interface crate to reduce dependencies of our
//! crate. Since all these types are a part of ckBTC public API, it is unlikely there will be
//! a breaking change in these types.

use candid::{CandidType, Deserialize, Principal};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_exports::ledger::Subaccount;
use serde::Serialize;

#[derive(Debug, CandidType, Deserialize)]
pub struct UpdateBalanceArgs {
    pub owner: Option<Principal>,
    pub subaccount: Option<Subaccount>,
}

/// The outcome of UTXO processing.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum UtxoStatus {
    /// The UTXO value does not cover the KYT check cost.
    ValueTooSmall(Utxo),
    /// The KYT check found issues with the deposited UTXO.
    Tainted(Utxo),
    /// The deposited UTXO passed the KYT check, but the minter failed to mint ckBTC on the ledger.
    /// The caller should retry the [update_balance] call.
    Checked(Utxo),
    /// The minter accepted the UTXO and minted ckBTC tokens on the ledger.
    Minted {
        /// The MINT transaction index on the ledger.
        block_index: u64,
        /// The minted amount (UTXO value minus fees).
        minted_amount: u64,
        /// The UTXO that caused the balance update.
        utxo: Utxo,
    },
}

#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum UpdateBalanceError {
    /// The minter experiences temporary issues, try the call again later.
    TemporarilyUnavailable(String),
    /// There is a concurrent [update_balance] invocation from the same caller.
    AlreadyProcessing,
    /// The minter didn't discover new UTXOs with enough confirmations.
    NoNewUtxos {
        /// If there are new UTXOs that do not have enough
        /// confirmations yet, this field will contain the number of
        /// confirmations as observed by the minter.
        current_confirmations: Option<u32>,
        /// The minimum number of UTXO confirmation required for the minter to accept a UTXO.
        required_confirmations: u32,
        /// List of utxos that don't have enough confirmations yet to be processed.
        pending_utxos: Option<Vec<PendingUtxo>>,
    },
    GenericError {
        error_code: u64,
        error_message: String,
    },
}

#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct PendingUtxo {
    pub outpoint: OutPoint,
    pub value: u64,
    pub confirmations: u32,
}

/// A reference to a transaction output.
#[derive(
    CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct OutPoint {
    /// A cryptographic hash of the transaction.
    /// A transaction can output multiple UTXOs.
    pub txid: Txid,
    /// The index of the output within the transaction.
    pub vout: u32,
}

#[derive(
    CandidType, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize,
)]
pub struct Txid(pub [u8; 32]);

impl<'a> TryFrom<&'a [u8]> for Txid {
    type Error = ();

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        let mut inner = [0; 32];
        inner.copy_from_slice(value);
        Ok(Self(inner))
    }
}
/// The arguments of the [retrieve_btc] endpoint.
///
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct RetrieveBtcArgs {
    // amount to retrieve in satoshi
    pub amount: u64,

    // address where to send bitcoins
    pub address: String,
}

#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct RetrieveBtcOk {
    // the index of the burn block on the ckbtc ledger
    pub block_index: u64,
}

#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum RetrieveBtcError {
    /// There is another request for this principal.
    AlreadyProcessing,

    /// The withdrawal amount is too low.
    AmountTooLow(u64),

    /// The bitcoin address is not valid.
    MalformedAddress(String),

    /// The withdrawal account does not hold the requested ckBTC amount.
    InsufficientFunds { balance: u64 },

    /// There are too many concurrent requests, retry later.
    TemporarilyUnavailable(String),

    /// A generic error reserved for future extensions.
    GenericError {
        error_message: String,
        /// See the [ErrorCode] enum above for the list of possible values.
        error_code: u64,
    },
}
