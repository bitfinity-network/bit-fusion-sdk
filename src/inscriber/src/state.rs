#![allow(unused)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use bitcoin::Amount;
use candid::{CandidType, Principal};
use did::{InscribeError, InscribeResult, InscriptionFees};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_log::{init_log, LogSettings};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use serde::Deserialize;

thread_local! {
    pub static RNG: RefCell<Option<StdRng>> = const { RefCell::new(None) };
    pub static BITCOIN_NETWORK: Cell<BitcoinNetwork> = const { Cell::new(BitcoinNetwork::Regtest) };
    pub static INSCRIBER_STATE: Rc<RefCell<State>> = Rc::default();
}

/// State of the Inscriber
#[derive(Default)]
pub struct State {
    config: InscriberConfig,
    utxos: Vec<UtxoManager>,
}

/// Configuration at canister initialization
#[derive(Debug, CandidType, Deserialize, Default)]
pub struct InscriberConfig {
    pub network: BitcoinNetwork,
    pub logger: LogSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UtxoManager {
    utxo: Utxo,
    purpose: UtxoType,
    amount: Amount,
}

/// Classification of UTXOs based on their purpose.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq)]
pub(crate) enum UtxoType {
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

impl State {
    /// Initializes the Inscriber's state with configuration information.
    pub(crate) fn configure(&mut self, config: InscriberConfig) {
        register_custom_getrandom();
        BITCOIN_NETWORK.with(|n| n.set(config.network));
        init_log(&config.logger).expect("Failed to initialize the logger");

        self.config = config;
    }

    /// Separates UTXOs into fees, inscriptions, and potential leftovers.
    ///
    /// Returns the UTXOs earmarked for inscription.
    pub(crate) fn process_utxos(
        &mut self,
        fees: InscriptionFees,
        mut fetched_utxos: Vec<Utxo>,
    ) -> InscribeResult<Vec<Utxo>> {
        let InscriptionFees {
            postage,
            commit_fee,
            reveal_fee,
        } = fees;
        let total_fees = postage + commit_fee + reveal_fee;

        // Sort UTXOs by value in ascending order to optimize for fee deduction.
        // This helps in using smaller UTXOs for fees, potentially leaving larger UTXOs for inscriptions.
        fetched_utxos.sort_by_key(|utxo| utxo.value);

        let total_utxo_amount = fetched_utxos.iter().map(|utxo| utxo.value).sum::<u64>();

        if total_utxo_amount < total_fees {
            return Err(InscribeError::InsufficientFundsForFees(format!(
                "Total UTXO amount: {}. Total fees required: {}",
                total_utxo_amount, total_fees
            )));
        }

        let mut accumulated_for_fees = 0u64;
        let mut remaining_utxos = Vec::new();

        for utxo in fetched_utxos.into_iter() {
            if accumulated_for_fees < total_fees {
                accumulated_for_fees += utxo.value;
                self.classify_utxo(&utxo, UtxoType::Fees, Amount::from_sat(utxo.value));
            } else {
                remaining_utxos.push(utxo.clone());
                self.classify_utxo(&utxo, UtxoType::Inscription, Amount::from_sat(utxo.value));
            }
        }

        if accumulated_for_fees > total_fees {
            if let Some(utxo) = self
                .utxos
                .iter_mut()
                .rev()
                .find(|u| u.purpose == UtxoType::Fees)
            {
                let leftover_value = accumulated_for_fees - total_fees;
                utxo.amount = Amount::from_sat(leftover_value);
                utxo.purpose = UtxoType::Leftover;
            }
        }

        let final_sum = remaining_utxos.iter().map(|utxo| utxo.value).sum::<u64>();
        if final_sum + total_fees < total_utxo_amount {
            return Err(InscribeError::InsufficientFundsForFees(format!(
                "Insufficient UTXOs for inscription after deducting fees. Available: {}, Required: {}",
                final_sum, total_utxo_amount - total_fees
            )));
        }

        Ok(remaining_utxos)
    }

    /// Classifies a new UTXO and adds it to the state.
    pub(crate) fn classify_utxo(&mut self, utxo: &Utxo, purpose: UtxoType, amount: Amount) {
        self.utxos.push(UtxoManager {
            utxo: utxo.clone(),
            purpose,
            amount,
        });
    }

    /// Selects UTXOs based on their purpose.
    pub(crate) fn select_utxos(&self, purpose: UtxoType) -> Vec<UtxoManager> {
        self.utxos
            .iter()
            .filter(|c_utxo| c_utxo.purpose == purpose)
            .cloned()
            .collect()
    }

    /// Updates the purpose of a UTXO after usage.
    pub(crate) fn update_utxo_purpose(&mut self, utxo_id: &str, new_purpose: UtxoType) {
        if let Some(utxo) = self.utxos.iter_mut().find(|c_utxo| {
            let txid = hex::encode(c_utxo.utxo.outpoint.txid.clone());
            txid == utxo_id
        }) {
            utxo.purpose = new_purpose;
            log::info!("UTXO updated: {:?}", utxo);
        } else {
            log::warn!("UTXO not found for updating: {}", utxo_id);
        }
    }
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
