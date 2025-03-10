use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Duration;

use bitcoin::consensus::Encodable;
use bitcoin::{Address, FeeRate, Transaction};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    bitcoin_get_current_fee_percentiles, bitcoin_get_utxos, bitcoin_send_transaction,
    BitcoinNetwork, GetCurrentFeePercentilesRequest, GetUtxosRequest, GetUtxosResponse,
    SendTransactionRequest,
};
use ic_exports::ic_kit::ic;

use crate::core::rune_inputs::GetInputsError;
use crate::interface::WithdrawError;

pub(crate) trait UtxoProvider {
    async fn get_utxos(&self, address: &Address) -> Result<GetUtxosResponse, GetInputsError>;
    async fn get_fee_rate(&self) -> Result<FeeRate, WithdrawError>;
    async fn send_tx(&self, transaction: &Transaction) -> Result<(), WithdrawError>;
}

type IcTimestamp = u64;

#[derive(Debug, PartialEq, Eq)]
struct UtxoCacheEntry {
    ts: IcTimestamp,
    address: Address,
    response: GetUtxosResponse,
}

impl PartialOrd<Self> for UtxoCacheEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UtxoCacheEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ts
            .cmp(&other.ts)
            .then(self.response.cmp(&other.response))
    }
}

thread_local! {
    static UTXO_CACHE: Rc<RefCell<BTreeSet<UtxoCacheEntry>>> = Default::default();
}

pub struct IcUtxoProvider {
    network: BitcoinNetwork,
    utxo_cache_timeout: Duration,
}

const DEFAULT_REGTEST_FEE: u64 = 100_000 * 1_000;

impl IcUtxoProvider {
    pub fn new(network: BitcoinNetwork, utxo_cache_timeout: Duration) -> Self {
        Self {
            network,
            utxo_cache_timeout,
        }
    }

    async fn request_utxos(&self, address: &Address) -> Result<GetUtxosResponse, GetInputsError> {
        let args = GetUtxosRequest {
            address: address.to_string(),
            network: self.network,
            filter: None,
        };

        log::trace!("Requesting UTXO list for address {address}");

        let response = bitcoin_get_utxos(args)
            .await
            .map(|value| value.0)
            .map_err(GetInputsError::btc)?;

        log::trace!("Got UTXO list result for address {address}:");
        log::trace!("{response:?}");

        Ok(response)
    }

    fn get_cached_utxos(&self, address: &Address) -> Option<GetUtxosResponse> {
        UTXO_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let invalidate_ts = ic::time() - self.utxo_cache_timeout.as_nanos() as u64;
            while !cache.is_empty() {
                if cache.first().expect("no entries").ts < invalidate_ts {
                    cache.pop_first();
                    continue;
                } else {
                    break;
                }
            }

            cache
                .iter()
                .find(|v| v.address == *address)
                .map(|v| v.response.clone())
        })
    }

    fn cache_utxo_response(&self, address: Address, response: GetUtxosResponse) {
        if self.utxo_cache_timeout == Duration::default() {
            return;
        }

        UTXO_CACHE.with(move |cache| {
            let mut cache = cache.borrow_mut();
            let curr_ts = ic::time();
            cache.insert(UtxoCacheEntry {
                ts: curr_ts,
                address,
                response,
            });
        });
    }
}

impl UtxoProvider for IcUtxoProvider {
    async fn get_utxos(&self, address: &Address) -> Result<GetUtxosResponse, GetInputsError> {
        match self.get_cached_utxos(address) {
            Some(v) => {
                log::trace!("UTXO list for address {address} found in cache");
                Ok(v)
            }
            None => {
                let response = self.request_utxos(address).await;
                if let Ok(resp) = &response {
                    self.cache_utxo_response(address.clone(), resp.clone());
                }

                response
            }
        }
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

        let middle_percentile = match self.network {
            BitcoinNetwork::Regtest => DEFAULT_REGTEST_FEE,
            _ if response.is_empty() => {
                log::error!("Empty response for fee rate request");
                return Err(WithdrawError::FeeRateRequest);
            }
            _ => response[response.len() / 2],
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

#[cfg(test)]
mod tests {
    use bitcoin::secp256k1::Secp256k1;
    use bitcoin::{Network, PublicKey};
    use ic_exports::ic_kit::MockContext;

    use super::*;

    fn address() -> Address {
        let s = Secp256k1::new();
        let public_key = PublicKey::new(s.generate_keypair(&mut rand::thread_rng()).1);

        Address::p2pkh(&public_key, Network::Bitcoin)
    }

    fn mock_response(v: u32) -> GetUtxosResponse {
        GetUtxosResponse {
            utxos: vec![],
            tip_block_hash: vec![],
            tip_height: v,
            next_page: None,
        }
    }

    #[test]
    fn cached_items_with_same_ts() {
        MockContext::new().inject();
        let provider = IcUtxoProvider::new(BitcoinNetwork::Mainnet, Duration::from_secs(1));
        let a1 = address();
        let r1 = mock_response(1);
        let a2 = address();
        let r2 = mock_response(2);

        provider.cache_utxo_response(a1.clone(), r1.clone());
        provider.cache_utxo_response(a2.clone(), r2.clone());

        assert_eq!(provider.get_cached_utxos(&a1), Some(r1));
        assert_eq!(provider.get_cached_utxos(&a2), Some(r2));
    }

    #[test]
    fn cached_items_are_removed() {
        let ctx = MockContext::new().inject();
        let provider = IcUtxoProvider::new(BitcoinNetwork::Mainnet, Duration::from_secs(60));
        let a1 = address();
        let r1 = mock_response(1);
        let a2 = address();
        let r2 = mock_response(2);
        let a3 = address();
        let r3 = mock_response(3);

        provider.cache_utxo_response(a1.clone(), r1.clone());
        ctx.add_time(Duration::from_secs(1).as_nanos() as u64);
        provider.cache_utxo_response(a2.clone(), r2.clone());
        ctx.add_time(Duration::from_secs(30).as_nanos() as u64);
        provider.cache_utxo_response(a3.clone(), r3.clone());
        ctx.add_time(Duration::from_secs(30).as_nanos() as u64);

        assert_eq!(provider.get_cached_utxos(&a1), None);
        assert_eq!(provider.get_cached_utxos(&a2), Some(r2));
        assert_eq!(provider.get_cached_utxos(&a3), Some(r3));
    }

    #[tokio::test]
    async fn get_utxos_returns_cached_value() {
        let ctx = MockContext::new().inject();
        let provider = IcUtxoProvider::new(BitcoinNetwork::Mainnet, Duration::from_secs(60));
        let a1 = address();
        let r1 = mock_response(1);

        provider.cache_utxo_response(a1.clone(), r1.clone());
        ctx.add_time(Duration::from_secs(30).as_nanos() as u64);

        assert_eq!(provider.get_utxos(&a1).await, Ok(r1));
    }

    #[tokio::test]
    #[should_panic(expected = "call_new should only be called inside canisters")]
    async fn get_utxos_requests_if_not_in_cache() {
        let ctx = MockContext::new().inject();
        let provider = IcUtxoProvider::new(BitcoinNetwork::Mainnet, Duration::from_secs(60));
        let a1 = address();
        let r1 = mock_response(1);
        let a2 = address();

        provider.cache_utxo_response(a1.clone(), r1.clone());
        ctx.add_time(Duration::from_secs(30).as_nanos() as u64);

        assert_eq!(provider.get_utxos(&a2).await, Ok(r1));
    }

    #[test]
    fn should_skip_caching_if_disabled() {
        MockContext::new().inject();
        let provider = IcUtxoProvider::new(BitcoinNetwork::Mainnet, Duration::from_secs(0));
        let a1 = address();
        let r1 = mock_response(1);
        let a2 = address();
        let r2 = mock_response(2);

        provider.cache_utxo_response(a1.clone(), r1.clone());
        provider.cache_utxo_response(a2.clone(), r2.clone());

        assert_eq!(provider.get_cached_utxos(&a1), None);
        assert_eq!(provider.get_cached_utxos(&a2), None);
    }
}
