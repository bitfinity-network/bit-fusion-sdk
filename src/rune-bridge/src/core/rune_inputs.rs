use std::collections::HashMap;

use bridge_did::runes::{RuneInfo, RuneName};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_exports::ic_kit::RejectionCode;
use thiserror::Error;

use crate::key::KeyError;
#[derive(Debug, Clone, Default)]
pub(crate) struct RuneInput {
    pub utxo: Utxo,
    pub runes: HashMap<RuneName, u128>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuneInputs {
    pub inputs: Vec<RuneInput>,
}

impl RuneInputs {
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    pub fn rune_amounts(&self) -> HashMap<RuneName, u128> {
        let mut rune_amounts = HashMap::new();
        for input in &self.inputs {
            for (rune_name, amount) in &input.runes {
                *rune_amounts.entry(*rune_name).or_default() += amount;
            }
        }

        rune_amounts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum GetInputsError {
    #[error("failed to connect to IC BTC adapter: {0}")]
    BtcAdapter(String),
    #[error("key error {0}")]
    KeyError(#[from] KeyError),
    #[error("indexer responded with an error: {0}")]
    IndexerError(String),
    #[error("rune indexers returned different result for same request: {first_response}; {another_response}")]
    IndexersDisagree {
        first_response: String,
        another_response: String,
    },
    #[error("insufficient consensus from rune indexers: {received_responses}/{required_responses} responses received, {checked_indexers} indexers checked")]
    InsufficientConsensus {
        received_responses: usize,
        required_responses: u8,
        checked_indexers: usize,
    },
}

impl GetInputsError {
    pub fn btc(call_error: (RejectionCode, String)) -> Self {
        Self::BtcAdapter(call_error.1)
    }
}

pub(crate) trait RuneInputProvider {
    async fn get_inputs(&self, dst_address: &H160) -> Result<RuneInputs, GetInputsError>;
    async fn get_rune_infos(
        &self,
        rune_amounts: &HashMap<RuneName, u128>,
    ) -> Option<Vec<(RuneInfo, u128)>>;
}

#[cfg(test)]
pub(crate) mod mock {
    use super::*;

    pub struct TestRuneInputProvider {
        inputs: Result<RuneInputs, GetInputsError>,
    }

    impl TestRuneInputProvider {
        pub fn empty() -> Self {
            Self {
                inputs: Ok(RuneInputs { inputs: vec![] }),
            }
        }

        pub fn err(err: GetInputsError) -> Self {
            Self { inputs: Err(err) }
        }

        pub fn with_input(input: RuneInput) -> Self {
            Self {
                inputs: Ok(RuneInputs {
                    inputs: vec![input],
                }),
            }
        }

        pub fn with_inputs(inputs: &[RuneInput]) -> Self {
            Self {
                inputs: Ok(RuneInputs {
                    inputs: inputs.into(),
                }),
            }
        }

        pub fn rune_info(&self, rune_name: &RuneName) -> RuneInfo {
            RuneInfo {
                name: *rune_name,
                decimals: 0,
                block: 0,
                tx: 0,
            }
        }
    }

    impl RuneInputProvider for TestRuneInputProvider {
        async fn get_inputs(&self, _dst_address: &H160) -> Result<RuneInputs, GetInputsError> {
            self.inputs.clone()
        }

        async fn get_rune_infos(
            &self,
            rune_amounts: &HashMap<RuneName, u128>,
        ) -> Option<Vec<(RuneInfo, u128)>> {
            Some(
                rune_amounts
                    .iter()
                    .map(|(name, amount)| (self.rune_info(name), *amount))
                    .collect(),
            )
        }
    }
}
