use std::cell::RefCell;
use std::rc::Rc;

use bitcoin::Address;
use candid::CandidType;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_log::{init_log, LogSettings};
use serde::Deserialize;

thread_local! {
    pub static INSCRIBER_STATE: Rc<RefCell<State>> = Rc::default();
}

pub fn get_inscriber_state() -> Rc<RefCell<State>> {
    INSCRIBER_STATE.with(|state| state.clone())
}

/// State of the Inscriber
#[derive(Default)]
pub struct State {
    config: InscriberConfig,
    // inscriptions: InscriptionWrapper
    own_addresses: Vec<Address>,
    own_utxos: Vec<Utxo>,
    utxo_types: UtxoType,
}

impl State {
    pub fn with_config(&mut self, config: InscriberConfig) {
        init_log(&config.logger).expect("Failed to initialize the logger");
        self.config = config;
    }

    pub fn own_addresses(&self) -> &[Address] {
        &self.own_addresses
    }

    pub fn own_utxos(&self) -> &[Utxo] {
        &self.own_utxos
    }

    pub fn utxo_type(&self) -> UtxoType {
        self.utxo_types
    }
}

/// Configuration at canister initialization
#[derive(Debug, CandidType, Deserialize, Default)]
pub struct InscriberConfig {
    pub network: BitcoinNetwork,
    pub logger: LogSettings,
}

/// Classification of UTXOs sent into the canister.
#[derive(Debug, Clone, Copy, CandidType, Deserialize, Default)]
pub enum UtxoType {
    /// UTXO earmarked for inscription.
    #[default]
    Inscription,
    /// UTXO used to pay for commit transaction
    CommitFee,
    /// UTXO used to pay for reveal transaction
    RevealFee,
    /// UTXOs left after fees have been deducted
    Leftover,
    /// UTXOs for a BRC-20 `transfer` inscription
    Transfer,
}
