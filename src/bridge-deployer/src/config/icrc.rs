use bridge_did::init::BridgeInitData;
use candid::Principal;
use clap::Parser;
use eth_signer::sign_strategy::SigningStrategy;
use serde::{Deserialize, Serialize};

use super::{LogCanisterSettings, SigningKeyId};
use crate::contracts::EvmNetwork;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct InitBridgeConfig {
    /// The signing key ID to use for signing messages
    ///
    /// This key are fixed in the management canister depending on the environment
    /// being used
    #[arg(long)]
    pub signing_key_id: SigningKeyId,
    /// Owner of the bridge canister
    #[arg(long)]
    pub owner: Principal,
    /// Log settings for the canister
    #[command(flatten, next_help_heading = "Log Settings for the canister")]
    pub log_settings: Option<LogCanisterSettings>,
}

impl InitBridgeConfig {
    /// Converts the `InitBridgeConfig` into a `BridgeInitData` struct.
    pub fn into_bridge_init_data(self, evm_network: EvmNetwork, evm: Principal) -> BridgeInitData {
        BridgeInitData {
            owner: self.owner,
            evm_link: crate::evm::evm_link(evm_network, Some(evm)),
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: self.signing_key_id.into(),
            },
            log_settings: self.log_settings.map(|v| ic_log::did::LogCanisterSettings {
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
            }),
        }
    }
}
