use std::time::Duration;

use bridge_did::init::IndexerType;
use clap::{Parser, ValueEnum};
use ic_exports::ic_cdk::api::management_canister::bitcoin;
use serde::{Deserialize, Serialize};

use super::LogCanisterSettings;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct RuneBridgeConfig {
    /// The network to use for the Bitcoin blockchain
    #[arg(long)]
    pub bitcoin_network: BitcoinNetwork,
    /// Specifies the period for which the result of BTC network requests would persist in the
    /// canister cache. If set to None or 0, the cache will not be used.
    #[arg(long)]
    pub btc_cache_timeout_secs: Option<u32>,
    /// The minimum number of confirmations required for a Bitcoin transaction
    /// to be considered final
    #[arg(long)]
    pub min_confirmations: u32,
    /// The threshold for the number of indexers required to reach consensus
    #[arg(long)]
    pub indexer_consensus_threshold: u8,
    /// The URLs of the indexers to use for the Bitcoin blockchain
    ///
    /// Note: The number of URLs must match the number of indexers specified above
    #[arg(long, value_delimiter = ',')]
    pub indexer_urls: Vec<String>,
    /// The fee to charge for deposits
    #[arg(long)]
    pub deposit_fee: u64,
    /// The timeout for the mempool to confirm a transaction
    #[arg(long)]
    pub mempool_timeout: u64,
    /// Log settings for the canister
    #[command(flatten, next_help_heading = "Log Settings for the canister")]
    pub log_settings: Option<LogCanisterSettings>,
}

#[derive(ValueEnum, Serialize, Deserialize, Debug, Clone)]
pub enum BitcoinNetwork {
    Mainnet,
    Testnet,
    Regtest,
}

impl From<BitcoinNetwork> for bitcoin::BitcoinNetwork {
    fn from(value: BitcoinNetwork) -> Self {
        match value {
            BitcoinNetwork::Mainnet => Self::Mainnet,
            BitcoinNetwork::Testnet => Self::Testnet,
            BitcoinNetwork::Regtest => Self::Regtest,
        }
    }
}

impl From<RuneBridgeConfig> for bridge_did::init::RuneBridgeConfig {
    fn from(value: RuneBridgeConfig) -> Self {
        Self {
            network: value.bitcoin_network.into(),
            btc_cache_timeout_secs: value.btc_cache_timeout_secs,
            min_confirmations: value.min_confirmations,
            indexers: value
                .indexer_urls
                .into_iter()
                .map(|url| IndexerType::OrdHttp { url })
                .collect(),
            deposit_fee: value.deposit_fee,
            mempool_timeout: Duration::from_secs(value.mempool_timeout),
            indexer_consensus_threshold: value.indexer_consensus_threshold,
        }
    }
}
