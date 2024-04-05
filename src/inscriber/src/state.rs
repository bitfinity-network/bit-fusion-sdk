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
use serde::{Deserialize, Serialize};

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
    network: BitcoinNetwork,
    logger: LogSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub(crate) struct UtxoManager {
    utxo: Utxo,
    purpose: UtxoType,
    value: Amount,
}

/// Classification of UTXOs based on their purpose.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq)]
pub(crate) enum UtxoType {
    /// UTXO earmarked for inscription.
    Inscription,
    /// UTXO used to pay for transaction fees
    #[default]
    Fees,
    /// UTXOs left after fees have been deducted
    Leftover,
    /// Indicates UTXOs that have been sent
    Spent,
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
        fetched_utxos.sort_unstable_by_key(|utxo| utxo.value);

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
                utxo.value = Amount::from_sat(leftover_value);
                utxo.purpose = UtxoType::Leftover;
            }
        }

        let final_sum = remaining_utxos.iter().map(|utxo| utxo.value).sum::<u64>();
        if final_sum < postage {
            return Err(InscribeError::InsufficientFundsForInscriptions(format!(
                "Insufficient UTXOs for inscription after deducting fees. Available: {}, Required: >= {}",
                final_sum, postage
            )));
        }

        Ok(remaining_utxos)
    }

    /// Updates the purpose of a UTXO after usage.
    pub(crate) fn update_utxo_purpose(&mut self, utxo_id: &str, new_purpose: UtxoType) {
        if let Some(utxo) = self.utxos.iter_mut().find(|c_utxo| {
            let txid_hex = hex::encode(c_utxo.utxo.outpoint.txid.clone());
            // Create a unique identifier for each UTXO in the format "txid_hex:vout"
            let txid_vout = format!("{}:{}", txid_hex, c_utxo.utxo.outpoint.vout);
            txid_vout == utxo_id
        }) {
            utxo.purpose = new_purpose;
            log::info!("UTXO updated: {:?}", utxo);
        } else {
            log::warn!("UTXO not found for updating: {}", utxo_id);
        }
    }

    /// Classifies a new UTXO and adds it to the state.
    fn classify_utxo(&mut self, utxo: &Utxo, purpose: UtxoType, value: Amount) {
        self.utxos.push(UtxoManager {
            utxo: utxo.clone(),
            purpose,
            value,
        });
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

#[cfg(test)]
mod tests {
    use ic_exports::ic_cdk::api::management_canister::bitcoin::Outpoint;
    use ord_rs::constants::POSTAGE;

    use super::*;

    fn get_mock_utxos() -> Vec<Utxo> {
        vec![
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0; 32],
                    vout: 0,
                },
                value: 50000, // satoshis
                height: 101,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![1; 32],
                    vout: 1,
                },
                value: 100000, // satoshis
                height: 102,
            },
        ]
    }

    #[test]
    fn process_utxos_enough_funds_for_fees_and_inscriptions() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 15000,
            reveal_fee: 10000,
        };

        let fetched_utxos = get_mock_utxos();
        let classified_utxos = state.process_utxos(fees, fetched_utxos).unwrap();

        // Expect that at least one UTXO is reserved for inscription
        assert!(!classified_utxos.is_empty());
        let total_spent = classified_utxos.iter().map(|utxo| utxo.value).sum::<u64>();
        assert!(total_spent >= POSTAGE);
    }

    #[test]
    fn process_utxos_insufficient_funds_for_inscriptions() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 100000,
            reveal_fee: 48000,
        };

        let fetched_utxos = get_mock_utxos();
        let classified_utxos = state.process_utxos(fees, fetched_utxos);

        assert!(matches!(
            classified_utxos,
            Err(InscribeError::InsufficientFundsForInscriptions(_))
        ));
    }

    #[test]
    fn process_utxos_insufficient_funds_for_fees() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 250000,
            reveal_fee: 150000,
        };

        let fetched_utxos = get_mock_utxos();
        let classified_utxos = state.process_utxos(fees, fetched_utxos);

        assert!(matches!(
            classified_utxos,
            Err(InscribeError::InsufficientFundsForFees(_))
        ));
    }

    #[test]
    fn update_utxo_purpose_after_use() {
        let mut state = State::default();
        let fetched_utxos = get_mock_utxos();
        for utxo in fetched_utxos.into_iter() {
            state.classify_utxo(&utxo, UtxoType::Fees, Amount::from_sat(utxo.value));
        }

        let txid_hex = hex::encode([0; 32]);
        let utxo_id = format!("{}:0", txid_hex);
        state.update_utxo_purpose(&utxo_id, UtxoType::Inscription);

        let updated_utxo = state
            .utxos
            .iter()
            .find(|um| {
                let txid_hex = hex::encode(um.utxo.outpoint.txid.clone());
                format!("{}:{}", txid_hex, um.utxo.outpoint.vout) == utxo_id
            })
            .expect("UTXO should exist");

        assert_eq!(updated_utxo.purpose, UtxoType::Inscription);
    }
}
