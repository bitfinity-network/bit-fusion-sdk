mod schnorr_key_id;

use std::collections::HashSet;
use std::time::Duration;

use candid::{CandidType, Decode, Encode};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{CellStructure, MemoryId, MemoryManager, StableCell, Storable};
use serde::Deserialize;

pub use self::schnorr_key_id::SchnorrKeyIds;
use super::{DEFAULT_DEPOSIT_FEE, DEFAULT_INDEXER_CONSENSUS_THRESHOLD, DEFAULT_MEMPOOL_TIMEOUT};
use crate::memory::CONFIG_MEMORY_ID;

pub struct Brc20BridgeConfigStorage<M: Memory> {
    config: StableCell<Brc20BridgeConfig, M>,
}

impl<M> Brc20BridgeConfigStorage<M>
where
    M: Memory,
{
    pub fn new(memory: &dyn MemoryManager<M, MemoryId>) -> Self {
        Self {
            config: StableCell::new(memory.get(CONFIG_MEMORY_ID), Brc20BridgeConfig::default())
                .expect("stable memory config initialization failed"),
        }
    }

    pub fn get(&self) -> &Brc20BridgeConfig {
        self.config.get()
    }

    pub fn set(&mut self, config: Brc20BridgeConfig) {
        self.config.set(config).expect("failed to set config");
    }

    pub fn with_borrow_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Brc20BridgeConfig),
    {
        let mut config = self.config.get().clone();

        f(&mut config);

        self.set(config);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct Brc20BridgeConfig {
    pub network: BitcoinNetwork,
    pub min_confirmations: u32,
    pub indexer_urls: HashSet<String>,
    pub deposit_fee: u64,
    pub mempool_timeout: Duration,
    /// Minimum quantity of indexer nodes required to reach agreement on a
    /// request
    pub indexer_consensus_threshold: u8,
    /// Schnorr key ID for the management canister
    pub schnorr_key_id: SchnorrKeyIds,
}

impl Storable for Brc20BridgeConfig {
    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Unbounded;

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Encode!(self)
            .expect("Failed to encode Brc20BridgeConfig")
            .into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(&bytes, Brc20BridgeConfig).expect("Failed to decode Brc20BridgeConfig")
    }
}

impl Default for Brc20BridgeConfig {
    fn default() -> Self {
        Self {
            network: BitcoinNetwork::Regtest,
            min_confirmations: 12,
            indexer_urls: HashSet::default(),
            deposit_fee: DEFAULT_DEPOSIT_FEE,
            mempool_timeout: DEFAULT_MEMPOOL_TIMEOUT,
            indexer_consensus_threshold: DEFAULT_INDEXER_CONSENSUS_THRESHOLD,
            schnorr_key_id: SchnorrKeyIds::TestKey1,
        }
    }
}

impl Brc20BridgeConfig {
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
        let config = Brc20BridgeConfig {
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
            schnorr_key_id: SchnorrKeyIds::TestKeyLocalDevelopment,
        };

        let bytes = config.to_bytes();
        let decoded = Brc20BridgeConfig::from_bytes(bytes.clone());

        assert_eq!(config, decoded);
    }

    #[test]
    fn test_should_encode_and_decode_config_with_empty_urls() {
        let config = Brc20BridgeConfig {
            network: BitcoinNetwork::Mainnet,
            min_confirmations: 12,
            indexer_urls: HashSet::new(),
            deposit_fee: 100,
            mempool_timeout: Duration::from_secs(60),
            indexer_consensus_threshold: 2,
            schnorr_key_id: SchnorrKeyIds::TestKeyLocalDevelopment,
        };

        let bytes = config.to_bytes();
        let decoded = Brc20BridgeConfig::from_bytes(bytes.clone());

        assert_eq!(config, decoded);
    }
}
