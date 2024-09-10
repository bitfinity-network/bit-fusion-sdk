use std::borrow::Cow;
use std::collections::HashSet;
use std::time::Duration;

use candid::{CandidType, Decode, Encode};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::Storable;
use serde::Deserialize;

use super::{DEFAULT_DEPOSIT_FEE, DEFAULT_INDEXER_CONSENSUS_THRESHOLD, DEFAULT_MEMPOOL_TIMEOUT};

#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct RuneBridgeConfig {
    pub network: BitcoinNetwork,

    /// Specifies the period for which the result of BTC network requests would persist in the
    /// canister cache. If set to None or 0, the cache will not be used.
    pub btc_cache_timeout_secs: Option<u32>,
    pub min_confirmations: u32,
    pub indexer_urls: HashSet<String>,
    pub deposit_fee: u64,
    pub mempool_timeout: Duration,
    /// Minimum quantity of indexer nodes required to reach agreement on a
    /// request
    pub indexer_consensus_threshold: u8,
}

impl Storable for RuneBridgeConfig {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = Encode!(self).expect("failed to encode rune config");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode rune config")
    }

    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Unbounded;
}

impl Default for RuneBridgeConfig {
    fn default() -> Self {
        Self {
            network: BitcoinNetwork::Regtest,
            btc_cache_timeout_secs: None,
            min_confirmations: 12,
            indexer_urls: HashSet::default(),
            deposit_fee: DEFAULT_DEPOSIT_FEE,
            mempool_timeout: DEFAULT_MEMPOOL_TIMEOUT,
            indexer_consensus_threshold: DEFAULT_INDEXER_CONSENSUS_THRESHOLD,
        }
    }
}

impl RuneBridgeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.indexer_urls.is_empty() {
            return Err("Indexer url is empty".to_string());
        }

        if self
            .indexer_urls
            .iter()
            .any(|url| !url.starts_with("https"))
        {
            return Err("Indexer url must specify https url".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_encode_and_decode_config() {
        let config = RuneBridgeConfig {
            network: BitcoinNetwork::Mainnet,
            btc_cache_timeout_secs: Some(300),
            min_confirmations: 12,
            indexer_urls: vec![
                "https://indexer1.com".to_string(),
                "https://indexer2.com".to_string(),
                "https://indexer3.com".to_string(),
            ]
            .into_iter()
            .collect(),
            deposit_fee: 100,
            mempool_timeout: Duration::from_secs(60),
            indexer_consensus_threshold: 2,
        };

        let bytes = config.to_bytes();
        let decoded = RuneBridgeConfig::from_bytes(bytes.clone());

        assert_eq!(config, decoded);
    }

    #[test]
    fn test_should_encode_and_decode_config_with_empty_urls() {
        let config = RuneBridgeConfig {
            network: BitcoinNetwork::Mainnet,
            btc_cache_timeout_secs: None,
            min_confirmations: 12,
            indexer_urls: HashSet::new(),
            deposit_fee: 100,
            mempool_timeout: Duration::from_secs(60),
            indexer_consensus_threshold: 2,
        };

        let bytes = config.to_bytes();
        let decoded = RuneBridgeConfig::from_bytes(bytes.clone());

        assert_eq!(config, decoded);
    }
}
