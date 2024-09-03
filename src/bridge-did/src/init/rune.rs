use std::collections::HashSet;
use std::time::Duration;

use candid::{CandidType, Principal};
use eth_signer::sign_strategy::SigningStrategy;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_log::did::LogCanisterSettings;
use serde::Deserialize;

use super::BridgeInitData;

pub const DEFAULT_DEPOSIT_FEE: u64 = 100_000;
pub const DEFAULT_MEMPOOL_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

/// Minimum number of indexers required to start the bridge.
pub const MIN_INDEXERS: u8 = 2;

#[derive(Debug, CandidType, Deserialize, Clone)]
pub struct RuneBridgeConfig {
    pub network: BitcoinNetwork,
    pub evm_principal: Principal,
    pub signing_strategy: SigningStrategy,
    pub admin: Principal,
    pub log_settings: LogCanisterSettings,
    pub min_confirmations: u32,
    pub no_of_indexers: u8,
    pub indexer_urls: HashSet<String>,
    /// Minimum quantity of indexer nodes required to reach agreement on a
    /// request
    pub indexer_consensus_threshold: u8,
    pub deposit_fee: u64,
    pub mempool_timeout: Duration,
}

impl Default for RuneBridgeConfig {
    fn default() -> Self {
        Self {
            network: BitcoinNetwork::Regtest,
            evm_principal: Principal::management_canister(),
            signing_strategy: SigningStrategy::Local {
                private_key: [0; 32],
            },
            admin: Principal::management_canister(),
            log_settings: LogCanisterSettings::default(),
            min_confirmations: 12,
            no_of_indexers: MIN_INDEXERS,
            indexer_consensus_threshold: 2,
            indexer_urls: HashSet::default(),
            deposit_fee: DEFAULT_DEPOSIT_FEE,
            mempool_timeout: DEFAULT_MEMPOOL_TIMEOUT,
        }
    }
}

impl RuneBridgeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.indexer_urls.is_empty() {
            return Err("Indexer url is empty".to_string());
        }

        if self.indexer_urls.len() != self.no_of_indexers as usize {
            return Err(format!(
                "Number of indexers ({}) required does not match number of indexer urls ({})",
                self.no_of_indexers,
                self.indexer_urls.len()
            ));
        }

        if self
            .indexer_urls
            .iter()
            .any(|url| !url.starts_with("https"))
        {
            return Err("Indexer url must specify https url".to_string());
        }

        if self.indexer_consensus_threshold > self.indexer_urls.len() as u8 {
            return Err(format!(
                "The consensus threshold ({}) must be less than or equal to the number of indexers ({})",
                self.indexer_consensus_threshold,
                self.indexer_urls.len()
            ));
        }

        Ok(())
    }

    pub fn bridge_init_data(&self) -> BridgeInitData {
        BridgeInitData {
            owner: self.admin,
            evm_principal: self.evm_principal,
            signing_strategy: self.signing_strategy.clone(),
            log_settings: Some(self.log_settings.clone()),
        }
    }
}
