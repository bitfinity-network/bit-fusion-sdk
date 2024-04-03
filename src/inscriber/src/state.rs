use std::cell::RefCell;
use std::rc::Rc;

use bitcoin::{Address, Amount};
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
    own_utxos: Vec<UtxoManager>,
    utxo_types: UtxoType,
}

impl State {
    /// Initializes the Inscriber's state with configuration.
    pub fn with_config(&mut self, config: InscriberConfig) {
        init_log(&config.logger).expect("Failed to initialize the logger");
        self.config = config;
    }

    /// Classifies a new UTXO and add it to the state.
    pub fn classify_utxo(&mut self, utxo: Utxo, purpose: UtxoType, amount: Amount) {
        let classified_utxo = UtxoManager {
            utxo,
            purpose,
            amount,
        };
        self.own_utxos.push(classified_utxo);
    }

    /// Selects UTXOs based on their purpose.
    pub fn select_utxos(&self, purpose: UtxoType) -> Vec<UtxoManager> {
        self.own_utxos
            .iter()
            .filter(|c_utxo| c_utxo.purpose == purpose)
            .cloned()
            .collect()
    }

    /// Updates the purpose of a UTXO after usage.
    pub fn update_utxo_purpose(&mut self, utxo_id: &str, new_purpose: UtxoType) {
        if let Some(utxo) = self.own_utxos.iter_mut().find(|c_utxo| {
            let txid = hex::encode(c_utxo.utxo.outpoint.txid.clone());
            txid == utxo_id
        }) {
            utxo.purpose = new_purpose;
            log::info!("UTXO updated: {:?}", utxo);
        } else {
            log::warn!("UTXO not found for updating: {}", utxo_id);
        }
    }

    pub fn own_addresses(&self) -> &[Address] {
        &self.own_addresses
    }

    pub fn own_utxos(&self) -> Vec<&Utxo> {
        self.own_utxos
            .iter()
            .map(|manager| &manager.utxo)
            .collect::<Vec<&Utxo>>()
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

/// Classification of UTXOs based on their purpose.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct UtxoManager {
    pub utxo: Utxo,
    pub purpose: UtxoType,
    pub amount: Amount,
}
