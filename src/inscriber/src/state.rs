use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use bitcoin::{Amount, Transaction};
use candid::{CandidType, Principal};
use did::{InscribeError, InscribeResult, InscriptionFees};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_log::{init_log, LogSettings};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};

thread_local! {
    pub(crate) static RNG: RefCell<Option<StdRng>> = const { RefCell::new(None) };
    pub(crate) static BITCOIN_NETWORK: Cell<BitcoinNetwork> = const { Cell::new(BitcoinNetwork::Regtest) };
    pub(crate) static INSCRIBER_STATE: Rc<RefCell<State>> = Rc::default();
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

/// Classification of a UTXO based on its purpose.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq)]
pub(crate) enum UtxoType {
    /// Denotes a UTXO earmarked for inscription.
    Inscription,
    /// Denotes a UTXO used to pay for transaction fees
    #[default]
    Fee,
    /// Denotes a UTXO left after fees have been deducted
    Leftover,
    /// Denotes a UTXO that has already been used in a
    /// previous inscription.
    Spent,
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
        fetched_utxos: Vec<Utxo>,
    ) -> InscribeResult<Vec<Utxo>> {
        let all_utxos = self.validate_utxos(fetched_utxos);

        let InscriptionFees {
            postage,
            commit_fee,
            reveal_fee,
        } = fees;
        let total_fees = postage + commit_fee + reveal_fee;

        let total_utxo_amount = all_utxos.iter().map(|utxo| utxo.value).sum::<u64>();

        if total_utxo_amount < total_fees {
            return Err(InscribeError::InsufficientFundsForFees(format!(
                "Total UTXO amount: {}. Total fees required: {}",
                total_utxo_amount, total_fees
            )));
        }

        let mut accumulated_for_fees = 0u64;
        let mut remaining_utxos = Vec::new();

        // Distribute UTXOs between fees, inscriptions, and leftovers.
        for utxo in all_utxos {
            let needed_for_fees = total_fees.saturating_sub(accumulated_for_fees);

            if needed_for_fees > 0 && utxo.value <= needed_for_fees {
                // This UTXO is entirely used for fees.
                accumulated_for_fees += utxo.value;
                self.classify_utxo(&utxo, UtxoType::Fee, Amount::from_sat(utxo.value));
            } else if needed_for_fees > 0 {
                // This UTXO covers the remaining fees and has leftovers.
                accumulated_for_fees += needed_for_fees;
                let leftover_value = utxo.value - needed_for_fees;
                self.classify_utxo(&utxo, UtxoType::Fee, Amount::from_sat(needed_for_fees));
                self.classify_utxo(&utxo, UtxoType::Leftover, Amount::from_sat(leftover_value));
            } else {
                // All fees are covered; the rest are for inscriptions.
                remaining_utxos.push(utxo.clone());
                self.classify_utxo(&utxo, UtxoType::Inscription, Amount::from_sat(utxo.value));
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

    /// Filters out any `UtxoType::Spent` UTXOs.
    ///
    /// This method first extracts all `UtxoType::Leftover` UTXOs,
    /// extending the collection with the newly-fetched UTXOs. This ensures that
    /// leftover funds are prioritized in the next transaction.
    fn validate_utxos(&mut self, utxos: Vec<Utxo>) -> Vec<Utxo> {
        // Extract leftover UTXOs from the state, if any.
        let mut all_utxos = self.extract_leftover_utxos();

        let utxos = utxos
            .into_iter()
            .filter(|utxo| {
                !self.utxos.iter().any(|managed_utxo| {
                    managed_utxo.utxo.outpoint == utxo.outpoint
                        && managed_utxo.purpose == UtxoType::Spent
                })
            })
            .collect::<Vec<Utxo>>();
        all_utxos.extend(utxos);

        // Sort UTXOs by value in ascending order to optimize for fee deduction.
        // This helps in using smaller UTXOs for fees, potentially leaving larger UTXOs for inscriptions.
        all_utxos.sort_unstable_by_key(|utxo| utxo.value);
        all_utxos
    }

    /// Retrieves (and consequently removes) leftover UTXOs from the state, returning them for re-use.
    ///
    /// This ensures that leftover funds are prioritized in the next transaction.
    fn extract_leftover_utxos(&mut self) -> Vec<Utxo> {
        let leftover_utxos = self
            .utxos
            .iter()
            .filter(|u| u.purpose == UtxoType::Leftover)
            .map(|u| u.utxo.clone())
            .collect::<Vec<_>>();

        // Remove the extracted leftovers from the state to prevent duplication.
        self.remove_utxos(UtxoType::Leftover);

        leftover_utxos
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

    /// Tags the given transaction's input UTXOs as `UtxoType::Spent`,
    /// effectively marking them for future removal.
    pub(crate) fn mark_utxos_as_spent(&mut self, tx: &Transaction) {
        for input in tx.input.iter() {
            let txid_hex = hex::encode(input.previous_output.txid);
            let utxo_id = format!("{}:{}", txid_hex, input.previous_output.vout);
            self.update_utxo_purpose(&utxo_id, UtxoType::Spent);
        }
    }

    /// Removes UTXOs identified by `UtxoType`.
    pub(crate) fn remove_utxos(&mut self, purpose: UtxoType) {
        self.utxos
            .retain(|utxo_manager| utxo_manager.purpose != purpose);
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

    fn get_mock_utxo(txid: &[u8; 32], vout: u32, value: u64) -> Utxo {
        Utxo {
            outpoint: Outpoint {
                txid: txid.to_vec(),
                vout,
            },
            value,
            height: 100,
        }
    }

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
            state.classify_utxo(&utxo, UtxoType::Fee, Amount::from_sat(utxo.value));
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

    #[test]
    fn process_utxos_with_all_funds_dedicated_to_fees() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 50000,
            reveal_fee: 50000,
        };
        let fetched_utxos = vec![get_mock_utxo(&[2; 32], 1, 100000)];

        let classified_utxos = state.process_utxos(fees, fetched_utxos);

        assert!(matches!(
            classified_utxos,
            Err(InscribeError::InsufficientFundsForFees(_))
        ));
    }

    #[test]
    fn process_utxos_exact_funds_for_fees_and_no_inscription() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 25000,
            reveal_fee: 25000,
        };
        let fetched_utxos = vec![get_mock_utxo(&[3; 32], 2, 50333)];

        assert!(state.process_utxos(fees, fetched_utxos).is_err());
    }

    #[test]
    fn process_utxos_leftovers_properly_allocated() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 15000,
            reveal_fee: 15000,
        };
        let fetched_utxos = vec![
            get_mock_utxo(&[4; 32], 3, 30333),
            get_mock_utxo(&[5; 32], 4, 50000),
            get_mock_utxo(&[6; 32], 5, 25000),
        ];

        let classified_utxos = state.process_utxos(fees, fetched_utxos).unwrap();

        assert!(
            !classified_utxos.is_empty(),
            "Should have UTXOs left for inscription"
        );
        // because we only need 1 for `UtxoType::Inscription`,
        // while the other 1 goes to `UtxoType::Leftover`
        assert_eq!(
            classified_utxos.len(),
            1,
            "Expected 1 UTXO to be left for inscription"
        );
    }

    #[test]
    fn insufficient_funds_for_any_transaction() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 100000,
            reveal_fee: 100000,
        };
        let fetched_utxos = vec![get_mock_utxo(&[7; 32], 6, 40000)];

        assert!(state.process_utxos(fees, fetched_utxos).is_err());
    }

    #[test]
    fn multiple_utxos_exact_funds_for_fees() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 20000,
            reveal_fee: 20000,
        };
        let fetched_utxos = vec![
            get_mock_utxo(&[8; 32], 7, POSTAGE),
            get_mock_utxo(&[9; 32], 8, 20000),
            get_mock_utxo(&[10; 32], 9, 20000),
        ];

        assert!(state.process_utxos(fees, fetched_utxos).is_err());
    }

    #[test]
    fn process_utxos_with_leftover_less_than_postage() {
        let mut state = State::default();

        let fees = InscriptionFees {
            postage: POSTAGE,
            commit_fee: 30000,
            reveal_fee: 30000,
        };
        let fetched_utxos = vec![
            get_mock_utxo(&[11; 32], 10, 45000),
            get_mock_utxo(&[12; 32], 11, 15444),
        ];

        let classified_utxos = state.process_utxos(fees, fetched_utxos);

        assert!(
            matches!(
                classified_utxos,
                Err(InscribeError::InsufficientFundsForInscriptions(_))
            ),
            "Should return an error when leftover funds are less than postage"
        );
    }
}
