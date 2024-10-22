use bridge_did::init::BridgeInitData;
use candid::Principal;
use clap::Parser;
use serde::{Deserialize, Serialize};

use super::{LogCanisterSettings, SigningKeyId};
use crate::contracts::EvmNetwork;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct InitBridgeConfig {
    /// The signing key ID to use for signing messages
    ///
    /// If not set, `production` or `dfx` signing key will be used based on the IC network the bridge
    /// is being deployed to.
    #[arg(long)]
    pub signing_key_id: Option<SigningKeyId>,
    /// Optional EVM canister to link to; if not provided, the default one will be used based on the network
    #[arg(long)]
    pub evm: Option<Principal>,
    /// Log settings for the canister
    #[command(flatten, next_help_heading = "Log Settings for the canister")]
    pub log_settings: Option<LogCanisterSettings>,
}

impl InitBridgeConfig {
    /// Converts the `InitBridgeConfig` into a `BridgeInitData` struct.
    pub fn into_bridge_init_data(
        self,
        owner: Principal,
        ic_host: &str,
        evm_network: EvmNetwork,
    ) -> BridgeInitData {
        let signing_strategy = self.signing_key_id(ic_host).into();
        let log_settings = self.log_settings.unwrap_or_else(default_log_settings);
        BridgeInitData {
            owner,
            evm_link: crate::evm::evm_link(evm_network, self.evm),
            signing_strategy,
            log_settings: Some(ic_log::did::LogCanisterSettings {
                enable_console: log_settings.enable_console,
                in_memory_records: log_settings.in_memory_records,
                max_record_length: log_settings.max_record_length,
                log_filter: log_settings.log_filter,
                acl: log_settings.acl.map(|v| {
                    v.iter()
                        .map(|(principal, perm)| {
                            (*principal, ic_log::did::LoggerPermission::from(*perm))
                        })
                        .collect()
                }),
            }),
        }
    }

    pub fn signing_key_id(&self, ic_host: &str) -> SigningKeyId {
        self.signing_key_id.unwrap_or_else(|| {
            if ic_host.starts_with("https://ic0.app") {
                SigningKeyId::Production
            } else {
                SigningKeyId::Dfx
            }
        })
    }
}

fn default_log_settings() -> LogCanisterSettings {
    LogCanisterSettings {
        enable_console: Some(true),
        in_memory_records: Some(10_000),
        max_record_length: Some(4096),
        log_filter: Some("trace,ic_task_scheduler=off".into()),
        acl: None,
    }
}
