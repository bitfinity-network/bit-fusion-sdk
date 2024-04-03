use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use bitcoin::{Address, Amount};
use candid::{CandidType, Principal};
use did::InscribeError;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_log::{init_log, LogSettings};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableCell, VirtualMemory};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use serde::Deserialize;

use crate::wallet::{bitcoin_api, inscription::InscriptionWrapper};

thread_local! {
    pub static RNG: RefCell<Option<StdRng>> = const { RefCell::new(None) };
    pub static BITCOIN_NETWORK: Cell<BitcoinNetwork> = const { Cell::new(BitcoinNetwork::Regtest) };
    pub static INSCRIBER_STATE: Rc<RefCell<State>> = Rc::default();
}

pub type Inscriptions = StableCell<InscriptionWrapper, VirtualMemory<DefaultMemoryImpl>>;

/// State of the Inscriber
#[derive(Default)]
pub struct State {
    config: InscriberConfig,
    own_utxos: Vec<UtxoManager>,
    utxo_type: UtxoType,
}

impl State {
    /// Initializes the Inscriber's state with configuration.
    pub fn configure(&mut self, config: InscriberConfig) {
        register_custom_getrandom();
        BITCOIN_NETWORK.with(|n| n.set(config.network));
        init_log(&config.logger).expect("Failed to initialize the logger");

        self.config = config;
    }

    // WIP
    #[allow(unused)]
    pub(crate) async fn fetch_and_manage_utxos(
        &mut self,
        own_address: Address,
    ) -> Result<Vec<Utxo>, InscribeError> {
        let network = self.config.network;

        // Fetch the UTXOs for the canister's address.
        log::info!("Fetching UTXOs for address: {}", own_address);
        let fetched_utxos = bitcoin_api::get_utxos(network, own_address.to_string())
            .await
            .map_err(InscribeError::FailedToCollectUtxos)?
            .utxos;

        let total_amount: u64 = fetched_utxos.iter().map(|utxo| utxo.value).sum();
        // TODO: Get inscription fees
        let fee_reserve_ratio = 0.1; // Reserve 10% of total for fees.
        let fees_amount = (total_amount as f64 * fee_reserve_ratio).round() as u64;
        let mut allocated_for_fees = 0u64;

        let mut utxos_for_inscription = Vec::new();

        for utxo in fetched_utxos {
            let purpose = if allocated_for_fees < fees_amount {
                allocated_for_fees += utxo.value;
                UtxoType::Fees
            } else {
                UtxoType::Inscription
            };

            let amount = Amount::from_sat(utxo.value);
            self.classify_utxo(utxo.clone(), purpose, amount);

            // If this UTXO is earmarked for inscription, add it to the return list.
            if purpose == UtxoType::Inscription {
                utxos_for_inscription.push(utxo);
            }
        }

        Ok(utxos_for_inscription)
    }

    /// Classifies a new UTXO and adds it to the state.
    pub fn classify_utxo(&mut self, utxo: Utxo, purpose: UtxoType, amount: Amount) {
        self.own_utxos.push(UtxoManager {
            utxo,
            purpose,
            amount,
        });
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

    pub fn utxo_type(&self) -> UtxoType {
        self.utxo_type
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
    /// UTXO used to pay for transaction fees
    Fees,
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

// In the following, we register a custom `getrandom` implementation because
// otherwise `getrandom` (which is an indirect dependency of `bitcoin`) fails to compile.
// This is necessary because `getrandom` by default fails to compile for the
// `wasm32-unknown-unknown` target (which is required for deploying a canister).
fn register_custom_getrandom() {
    ic_exports::ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_exports::ic_cdk::spawn(set_rand())
    });
    getrandom::register_custom_getrandom!(custom_rand);
}

fn custom_rand(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    RNG.with(|rng| rng.borrow_mut().as_mut().unwrap().fill_bytes(buf));
    Ok(())
}

async fn set_rand() {
    let (seed,) = ic_exports::ic_cdk::call(Principal::management_canister(), "raw_rand", ())
        .await
        .unwrap();
    RNG.with(|rng| {
        *rng.borrow_mut() = Some(StdRng::from_seed(seed));
        log::debug!("rng: {:?}", *rng.borrow());
    });
}
