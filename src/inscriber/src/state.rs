// WIP
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use bitcoin::{Address, Amount};
use candid::{CandidType, Principal};
use did::{InscribeError, InscribeResult, InscriptionFees};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_log::{init_log, LogSettings};
use ord_rs::MultisigConfig;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use serde::Deserialize;

use crate::wallet::inscription::Protocol;
use crate::wallet::{bitcoin_api, CanisterWallet};

thread_local! {
    pub static RNG: RefCell<Option<StdRng>> = const { RefCell::new(None) };
    pub static BITCOIN_NETWORK: Cell<BitcoinNetwork> = const { Cell::new(BitcoinNetwork::Regtest) };
    pub static INSCRIBER_STATE: Rc<RefCell<State>> = Rc::default();
}

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

    /// Retrieves the UTXOs for an address and classifies each according to the purpose.
    pub(crate) async fn fetch_and_classify_utxos(
        &mut self,
        own_address: Address,
        inscription_type: Protocol,
        inscription: String,
        multisig_config: Option<MultisigConfig>,
    ) -> InscribeResult<ClassifiedUtxos> {
        let network = self.config.network;
        let InscriptionFees {
            postage,
            commit_fee,
            reveal_fee,
        } = CanisterWallet::new(vec![], network)
            .get_inscription_fees(inscription_type, inscription, multisig_config)
            .await?;

        // Fetch the UTXOs for the canister's address.
        log::info!("Fetching UTXOs for address: {}", own_address);
        let fetched_utxos = self.fetch_utxos(own_address, network).await?;

        let total_amount: u64 = fetched_utxos.iter().map(|utxo| utxo.value).sum();
        let total_fees = postage + commit_fee + reveal_fee;

        // Check if total UTXOs cover all fees + at least a minimal amount for inscription.
        if total_amount < total_fees {
            return Err(InscribeError::InsufficientFundsForFees(format!(
                "Total amount: {total_amount}. Total fees: {total_fees}"
            )));
        }

        let mut allocated_for_fees = 0u64;
        let mut utxos_for_inscription = Vec::new();
        let mut utxos_for_fees = Vec::new();

        for utxo in fetched_utxos {
            // Prioritize allocation to fees until all fees are covered.
            if allocated_for_fees < total_fees {
                allocated_for_fees += utxo.value;
                utxos_for_fees.push(utxo.clone());
                self.classify_utxo(&utxo, UtxoType::Fees, Amount::from_sat(utxo.value));
            } else {
                utxos_for_inscription.push(utxo.clone());
                self.classify_utxo(&utxo, UtxoType::Inscription, Amount::from_sat(utxo.value));
            }
        }

        // Check if the separated UTXOs for fees adequately cover all required fees.
        let fees_covered = utxos_for_fees.iter().map(|utxo| utxo.value).sum::<u64>() >= total_fees;
        if !fees_covered {
            return Err(InscribeError::InsufficientFundsForFees(format!(
                "Total amount: {total_amount}. Total fees: {total_fees}"
            )));
        }

        Ok(ClassifiedUtxos {
            inscriptions: utxos_for_inscription,
            fees: utxos_for_fees,
            leftovers: vec![],
        })
    }

    /// Classifies a new UTXO and adds it to the state.
    pub(crate) fn classify_utxo(&mut self, utxo: &Utxo, purpose: UtxoType, amount: Amount) {
        self.own_utxos.push(UtxoManager {
            utxo: utxo.clone(),
            purpose,
            amount,
        });
    }

    /// Selects UTXOs based on their purpose.
    pub(crate) fn select_utxos(&self, purpose: UtxoType) -> Vec<UtxoManager> {
        self.own_utxos
            .iter()
            .filter(|c_utxo| c_utxo.purpose == purpose)
            .cloned()
            .collect()
    }

    /// Updates the purpose of a UTXO after usage.
    pub(crate) fn update_utxo_purpose(&mut self, utxo_id: &str, new_purpose: UtxoType) {
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

    async fn fetch_utxos(
        &mut self,
        own_address: Address,
        network: BitcoinNetwork,
    ) -> InscribeResult<Vec<Utxo>> {
        Ok(bitcoin_api::get_utxos(network, own_address.to_string())
            .await
            .map_err(InscribeError::FailedToCollectUtxos)?
            .utxos)
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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ClassifiedUtxos {
    pub(crate) inscriptions: Vec<Utxo>,
    pub(crate) fees: Vec<Utxo>,
    pub(crate) leftovers: Vec<Utxo>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UtxoManager {
    utxo: Utxo,
    purpose: UtxoType,
    amount: Amount,
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
