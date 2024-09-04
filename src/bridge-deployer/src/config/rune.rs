use std::time::Duration;

use candid::Principal;
use clap::{Parser, ValueEnum};
use eth_signer::sign_strategy::SigningStrategy;
use ic_exports::ic_cdk::api::management_canister::bitcoin;
use serde::{Deserialize, Serialize};

use super::{LogCanisterSettings, SigningKeyId};

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct RuneBridgeConfig {
    /// The network to use for the Bitcoin blockchain
    #[arg(long)]
    pub bitcoin_network: BitcoinNetwork,
    /// The principal of the EVM canister that is being deployed
    #[arg(long)]
    pub evm_principal: Principal,
    /// The signing key ID to use for signing messages
    #[arg(long, default_value_t = SigningKeyId::Test)]
    pub signing_key_id: SigningKeyId,
    /// Admin of the bridge canister
    #[arg(long)]
    pub admin: Principal,
    /// The minimum number of confirmations required for a Bitcoin transaction
    /// to be considered final
    #[arg(long)]
    pub min_confirmations: u32,
    /// Minimum quantity of indexer nodes required to reach agreement on a
    /// request
    pub indexer_consensus_threshold: u8,
    /// The number of indexers to use for the Bitcoin blockchain
    #[arg(long)]
    pub no_of_indexers: u8,
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
            evm_principal: value.evm_principal,
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: value.signing_key_id.into(),
            },
            admin: value.admin,
            log_settings: value
                .log_settings
                .map(|v| ic_log::did::LogCanisterSettings {
                    enable_console: v.enable_console,
                    in_memory_records: v.in_memory_records,
                    max_record_length: v.max_record_length,
                    log_filter: v.log_filter,
                    acl: v.acl.map(|v| {
                        v.iter()
                            .map(|(principal, perm)| {
                                (*principal, ic_log::did::LoggerPermission::from(*perm))
                            })
                            .collect()
                    }),
                })
                .unwrap_or_default(),
            min_confirmations: value.min_confirmations,
            no_of_indexers: value.no_of_indexers,
            indexer_urls: value.indexer_urls.into_iter().collect(),
            deposit_fee: value.deposit_fee,
            mempool_timeout: Duration::from_secs(value.mempool_timeout),
            indexer_consensus_threshold: value.indexer_consensus_threshold,
        }
    }
}
