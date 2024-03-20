use candid::{CandidType, Principal};
use ic_btc_interface::{OutPoint, Utxo};
use ic_exports::ic_cdk::api::management_canister::main::CanisterId;
use serde::{Deserialize, Serialize};

// For source see: https://github.com/dfinity/ic/blob/master/rs/bitcoin/ckbtc/minter/src/lifecycle/init.rs

#[derive(CandidType, serde::Deserialize)]
pub enum MinterArg {
    Init(InitArgs),
    Upgrade(Option<UpgradeArgs>),
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InitArgs {
    /// The bitcoin network that the minter will connect to
    pub btc_network: BtcNetwork,

    /// The name of the [EcdsaKeyId]. Use "dfx_test_key" for local replica and "test_key_1" for
    /// a testing key for testnet and mainnet
    pub ecdsa_key_name: String,

    /// Minimum amount of bitcoin that can be retrieved
    pub retrieve_btc_min_amount: u64,

    /// The CanisterId of the ckBTC Ledger
    pub ledger_id: CanisterId,

    /// Maximum time in nanoseconds that a transaction should spend in the queue
    /// before being sent.
    pub max_time_in_queue_nanos: u64,

    /// Specifies the minimum number of confirmations on the Bitcoin network
    /// required for the minter to accept a transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confirmations: Option<u32>,

    /// The mode controlling access to the minter.
    #[serde(default)]
    pub mode: Mode,

    /// The fee that the minter will pay for each KYT check.
    /// NOTE: this field is optional for backward compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyt_fee: Option<u64>,

    /// The principal of the KYT canister.
    /// NOTE: this field is optional for backward compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyt_principal: Option<CanisterId>,
}

#[derive(CandidType, Clone, Copy, Deserialize, Debug, Eq, PartialEq, Serialize, Hash)]
pub enum BtcNetwork {
    Mainnet,
    Testnet,
    Regtest,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct UpgradeArgs {
    /// Minimum amount of bitcoin that can be retrieved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieve_btc_min_amount: Option<u64>,

    /// Specifies the minimum number of confirmations on the Bitcoin network
    /// required for the minter to accept a transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confirmations: Option<u32>,

    /// Maximum time in nanoseconds that a transaction should spend in the queue
    /// before being sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_time_in_queue_nanos: Option<u64>,

    /// The mode in which the minter is running.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyt_fee: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyt_principal: Option<CanisterId>,
}

/// Controls which operations the minter can perform.
#[derive(
    Default, candid::CandidType, Clone, Debug, PartialEq, Eq, serde::Deserialize, Serialize,
)]
pub enum Mode {
    /// Minter's state is read-only.
    ReadOnly,
    /// Only the specified principals can modify the minter's state.
    RestrictedTo(Vec<Principal>),
    /// Only the specified principals can deposit BTC.
    DepositsRestrictedTo(Vec<Principal>),
    /// No restrictions on the minter interactions.
    #[default]
    GeneralAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub enum LifecycleArg {
    InitArg(InitArg),
    // UpgradeArg(UpgradeArg),
}

#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct InitArg {
    /// The principal of the minter canister.
    pub minter_id: Principal,
    /// The list of callers who can update the API key.
    pub maintainers: Vec<Principal>,
    /// The mode in which this canister runs.
    pub mode: KytMode,
}

#[derive(Debug, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum KytMode {
    /// In this mode, the canister will not make any HTTP calls and return empty
    /// alert lists for all requests.
    AcceptAll,
    /// In this mode, the canister will mark generate bogus alerts for all requests.
    RejectAll,
    /// In this mode, the canister will call Chainalysis API for each request.
    Normal,
}

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
