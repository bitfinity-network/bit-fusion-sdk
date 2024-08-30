use bridge_did::init::BridgeInitData;
use candid::Principal;
use clap::Parser;
use icrc2_bridge::SigningStrategy;
use serde::{Deserialize, Serialize};

use super::{LogCanisterSettings, SigningKeyId};

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct InitBridgeConfig {
    /// The principal of the EVM canister that is being deployed
    #[arg(long)]
    pub evm_principal: Principal,
    /// The signing key ID to use for signing messages
    ///
    /// This key are fixed in the management canister depending on the environment
    /// being used
    #[arg(long)]
    pub signing_key_id: SigningKeyId,
    //// Owner of the bridge cansiter
    #[arg(long)]
    pub owner: Principal,
    /// Log settings for the canister
    #[command(flatten, next_help_heading = "Log Settings for the canister")]
    pub log_settings: Option<LogCanisterSettings>,
}

impl From<InitBridgeConfig> for BridgeInitData {
    fn from(value: InitBridgeConfig) -> Self {
        BridgeInitData {
            owner: value.owner,
            evm_principal: value.evm_principal,
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: value.signing_key_id.into(),
            },
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
                }),
        }
    }
}
