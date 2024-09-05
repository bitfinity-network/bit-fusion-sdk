use bridge_did::order::MintOrder;
use bridge_did::runes::RuneToWrap;
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum UtxoHandlerError {
    #[error("failed to connect to IC BTC adapter: {0}")]
    BtcAdapter(String),
    #[error("requested utxo is not in the main branch")]
    UtxoNotFound,
    #[error("utxo is not confirmed, required {required_confirmations}, currently {current_confirmations} confirmations")]
    NotConfirmed {
        required_confirmations: u32,
        current_confirmations: u32,
    },
    #[error("utxo is already used to create mint orders")]
    UtxoAlreadyUsed,
}

pub(crate) trait UtxoHandler {
    async fn check_confirmations(
        &self,
        dst_address: &H160,
        utxo: &Utxo,
    ) -> Result<(), UtxoHandlerError>;

    async fn deposit(
        &self,
        utxo: &Utxo,
        dst_address: &H160,
        utxo_runes: Vec<RuneToWrap>,
    ) -> Result<Vec<MintOrder>, UtxoHandlerError>;
}

#[cfg(test)]
pub mod test {
    use bridge_did::id256::Id256;

    use super::*;

    pub(crate) struct TestUtxoHandler {
        check_result: Result<(), UtxoHandlerError>,
        is_utxo_used: bool,
    }

    impl TestUtxoHandler {
        pub fn with_error(err: UtxoHandlerError) -> Self {
            Self {
                check_result: Err(err),
                is_utxo_used: false,
            }
        }

        pub fn ok() -> Self {
            Self {
                check_result: Ok(()),
                is_utxo_used: false,
            }
        }

        pub fn already_used_utxo() -> Self {
            Self {
                check_result: Ok(()),
                is_utxo_used: true,
            }
        }
    }

    impl UtxoHandler for TestUtxoHandler {
        async fn check_confirmations(
            &self,
            _dst_address: &H160,
            _utxo: &Utxo,
        ) -> Result<(), UtxoHandlerError> {
            self.check_result.clone()
        }

        async fn deposit(
            &self,
            _utxo: &Utxo,
            _dst_address: &H160,
            utxo_runes: Vec<RuneToWrap>,
        ) -> Result<Vec<MintOrder>, UtxoHandlerError> {
            if self.is_utxo_used {
                Err(UtxoHandlerError::UtxoAlreadyUsed)
            } else {
                let mint_orders = utxo_runes
                    .into_iter()
                    .map(|_rune| MintOrder {
                        amount: Default::default(),
                        sender: Id256::from_evm_address(&H160::from_slice(&[1; 20]), 1),
                        src_token: Id256::from_evm_address(&H160::from_slice(&[2; 20]), 1),
                        recipient: Default::default(),
                        dst_token: Default::default(),
                        nonce: 0,
                        sender_chain_id: 0,
                        recipient_chain_id: 0,
                        name: [1; 32],
                        symbol: [1; 16],
                        decimals: 0,
                        approve_spender: Default::default(),
                        approve_amount: Default::default(),
                        fee_payer: Default::default(),
                    })
                    .collect();

                Ok(mint_orders)
            }
        }
    }
}
