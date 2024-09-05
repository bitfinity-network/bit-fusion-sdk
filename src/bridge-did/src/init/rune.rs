use std::collections::HashSet;
use std::time::Duration;

use candid::CandidType;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::Storable;
use serde::Deserialize;

use super::{DEFAULT_DEPOSIT_FEE, DEFAULT_INDEXER_CONSENSUS_THRESHOLD, DEFAULT_MEMPOOL_TIMEOUT};

#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct RuneBridgeConfig {
    pub network: BitcoinNetwork,
    pub min_confirmations: u32,
    pub indexer_urls: HashSet<String>,
    pub deposit_fee: u64,
    pub mempool_timeout: Duration,
    /// Minimum quantity of indexer nodes required to reach agreement on a
    /// request
    pub indexer_consensus_threshold: u8,
}

impl Storable for RuneBridgeConfig {
    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Unbounded;

    /* Encoding
       1                                            // network
       4                                            // min_confirmations
       1                                            // number of indexers
       number of indexers * [1 + indexer_url.len]   // [len] + [indexer_url]
       8                                            // deposit_fee
       8                                            // mempool_timeout
       1                                            // indexer_consensus_threshold
    */

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let mut buf = Vec::with_capacity(
            1 + 4
                + 1
                + self
                    .indexer_urls
                    .iter()
                    .map(|url| 1 + url.len())
                    .sum::<usize>()
                + 8
                + 8
                + 1,
        );

        let network_byte = match self.network {
            BitcoinNetwork::Mainnet => 0,
            BitcoinNetwork::Testnet => 1,
            BitcoinNetwork::Regtest => 2,
        };

        buf.push(network_byte);
        buf.extend_from_slice(&self.min_confirmations.to_le_bytes());
        buf.push(self.indexer_urls.len() as u8);
        for url in &self.indexer_urls {
            buf.push(url.len() as u8);
            buf.extend_from_slice(url.as_bytes());
        }
        buf.extend_from_slice(&self.deposit_fee.to_le_bytes());
        buf.extend_from_slice(&(self.mempool_timeout.as_nanos() as u64).to_le_bytes());
        buf.push(self.indexer_consensus_threshold);

        buf.into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let mut offset = 0;
        let network = bytes[offset];
        offset += 1;
        let min_confirmations = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let no_of_indexers = bytes[offset];
        offset += 1;
        let mut indexer_urls = HashSet::with_capacity(no_of_indexers as usize);
        for _ in 0..no_of_indexers {
            let len = bytes[offset] as usize;
            offset += 1;
            let url =
                String::from_utf8(bytes[offset..offset + len].to_vec()).expect("invalid utf8");
            offset += len;
            indexer_urls.insert(url);
        }
        let deposit_fee = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let mempool_timeout = Duration::from_nanos(u64::from_le_bytes(
            bytes[offset..offset + 8].try_into().unwrap(),
        ));
        offset += 8;
        let indexer_consensus_threshold = bytes[offset];

        Self {
            network: match network {
                0 => BitcoinNetwork::Mainnet,
                1 => BitcoinNetwork::Testnet,
                2 => BitcoinNetwork::Regtest,
                _ => panic!("invalid network"),
            },
            min_confirmations,
            indexer_urls,
            deposit_fee,
            mempool_timeout,
            indexer_consensus_threshold,
        }
    }
}

impl Default for RuneBridgeConfig {
    fn default() -> Self {
        Self {
            network: BitcoinNetwork::Regtest,
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
