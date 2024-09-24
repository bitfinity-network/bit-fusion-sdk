use std::str::FromStr;

use bridge_did::init::BridgeInitData;
use candid::{CandidType, Principal};
use clap::Parser;
use eth_signer::sign_strategy::SigningStrategy;
use serde::{Deserialize, Serialize};

use super::{LogCanisterSettings, SigningKeyId};

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct InitBridgeConfig {
    /// Parameters of connecting to the EVM.
    ///
    /// If the value is an IC principal, direct connection to EVM canister will be used. Otherwise,
    /// the value is considered to be an URL to the EVM HTTP RPC server.
    #[arg(long)]
    pub evm_link: EvmLink,
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

impl From<InitBridgeConfig> for BridgeInitData {
    fn from(value: InitBridgeConfig) -> Self {
        BridgeInitData {
            owner: value.owner,
            evm_link: value.evm_link.into(),
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

#[derive(CandidType, Debug, Serialize, Deserialize, Clone)]
pub enum EvmLink {
    /// Direct connection to EVM canister with inter-canister calls.
    Ic(Principal),
    /// Connection to an EVM with HTTP RPC calls.
    Http(String),
}

impl From<EvmLink> for bridge_did::evm_link::EvmLink {
    fn from(value: EvmLink) -> Self {
        match value {
            EvmLink::Ic(principal) => bridge_did::evm_link::EvmLink::Ic(principal),
            EvmLink::Http(url) => bridge_did::evm_link::EvmLink::Http(url),
        }
    }
}

impl From<String> for EvmLink {
    fn from(value: String) -> Self {
        if let Ok(principal) = Principal::from_str(&value) {
            Self::Ic(principal)
        } else {
            Self::Http(value)
        }
    }
}
