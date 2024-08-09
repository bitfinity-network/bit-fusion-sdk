use bitcoin::consensus::Encodable;
use bitcoin::{Address, FeeRate, Transaction};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    bitcoin_get_current_fee_percentiles, bitcoin_get_utxos, bitcoin_send_transaction,
    BitcoinNetwork, GetCurrentFeePercentilesRequest, GetUtxosRequest, GetUtxosResponse,
    SendTransactionRequest,
};

use crate::interface::{DepositError, WithdrawError};

pub(crate) trait UtxoProvider {
    async fn get_utxos(&self, address: &Address) -> Result<GetUtxosResponse, DepositError>;
    async fn get_fee_rate(&self) -> Result<FeeRate, WithdrawError>;
    async fn send_tx(&self, transaction: &Transaction) -> Result<(), WithdrawError>;
}

pub struct IcUtxoProvider {
    network: BitcoinNetwork,
}

const DEFAULT_REGTEST_FEE: u64 = 10_000;

impl IcUtxoProvider {
    pub fn new(network: BitcoinNetwork) -> Self {
        Self { network }
    }
}

impl UtxoProvider for IcUtxoProvider {
    async fn get_utxos(&self, address: &Address) -> Result<GetUtxosResponse, DepositError> {
        let args = GetUtxosRequest {
            address: address.to_string(),
            network: self.network,
            filter: None,
        };

        log::trace!("Requesting UTXO list for address {address}");

        let response = bitcoin_get_utxos(args)
            .await
            .map(|value| value.0)
            .map_err(|err| {
                DepositError::Unavailable(format!(
                    "Unexpected response from management canister: {err:?}"
                ))
            })?;

        log::trace!("Got UTXO list result for address {address}:");
        log::trace!("{response:?}");

        Ok(response)
    }

    async fn get_fee_rate(&self) -> Result<FeeRate, WithdrawError> {
        let args = GetCurrentFeePercentilesRequest {
            network: self.network,
        };
        let response = bitcoin_get_current_fee_percentiles(args)
            .await
            .map_err(|err| {
                log::error!("Failed to get current fee rate: {err:?}");
                WithdrawError::FeeRateRequest
            })?
            .0;

        let middle_percentile = if response.is_empty() {
            match self.network {
                BitcoinNetwork::Regtest => DEFAULT_REGTEST_FEE * 1000,
                _ => {
                    log::error!("Empty response for fee rate request");
                    return Err(WithdrawError::FeeRateRequest);
                }
            }
        } else {
            response[response.len() / 2]
        };

        log::trace!("Received fee rate percentiles: {response:?}");

        log::info!("Using fee rate {}", middle_percentile / 1000);

        FeeRate::from_sat_per_vb(middle_percentile / 1000).ok_or_else(|| {
            log::error!("Invalid fee rate received from IC: {middle_percentile}");
            WithdrawError::FeeRateRequest
        })
    }

    async fn send_tx(&self, transaction: &Transaction) -> Result<(), WithdrawError> {
        log::trace!(
            "Sending transaction {} to the bitcoin adapter",
            transaction.txid()
        );

        let mut serialized = vec![];
        transaction
            .consensus_encode(&mut serialized)
            .map_err(|err| {
                log::error!("Failed to serialize transaction: {err:?}");
                WithdrawError::TransactionSerialization
            })?;

        log::trace!(
            "Serialized transaction {}: {}",
            transaction.txid(),
            hex::encode(&serialized)
        );

        let request = SendTransactionRequest {
            transaction: serialized,
            network: self.network,
        };
        bitcoin_send_transaction(request).await.map_err(|err| {
            log::error!("Failed to send transaction: {err:?}");
            WithdrawError::TransactionSending
        })?;

        log::trace!("Transaction {} sent to the adapter", transaction.txid());

        Ok(())
    }
}
