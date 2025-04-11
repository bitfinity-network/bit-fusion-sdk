use std::collections::HashSet;

pub use brc20::Brc20BridgeConfig;
use candid::{CandidType, Principal};
use clap::{Parser, ValueEnum};
use eth_signer::sign_strategy::SigningStrategy;
use eth_signer::{sign_strategy, LocalWallet};
use serde::{Deserialize, Serialize};

mod brc20;
mod btc;
mod erc;
mod init;
mod rune;

pub use btc::*;
pub use erc::*;
pub use init::*;
pub use rune::*;

#[derive(
    ValueEnum, Debug, Serialize, Deserialize, Clone, Copy, CandidType, PartialEq, Eq, strum::Display,
)]
/// The signing key ID to use for signing messages
///
/// This key are fixed in the management canister depending on the environment
pub enum SigningKeyId {
    /// The DFX signing key
    Dfx,
    /// The test signing key
    Test,
    Production,
    Pk,
}

impl From<SigningKeyId> for SigningStrategy {
    fn from(value: SigningKeyId) -> Self {
        match value {
            SigningKeyId::Dfx => Self::ManagementCanister {
                key_id: sign_strategy::SigningKeyId::Dfx,
            },
            SigningKeyId::Test => Self::ManagementCanister {
                key_id: sign_strategy::SigningKeyId::Test,
            },
            SigningKeyId::Production => Self::ManagementCanister {
                key_id: sign_strategy::SigningKeyId::Production,
            },
            SigningKeyId::Pk => {
                let signer = LocalWallet::random();
                let pk = signer.to_bytes();
                Self::Local {
                    private_key: pk.into(),
                }
            }
        }
    }
}

#[derive(
    ValueEnum, Debug, Clone, Copy, Serialize, CandidType, Deserialize, Eq, PartialEq, Hash,
)]
/// Logger permission for the ACL
pub enum LoggerPermission {
    Read,
    Configure,
}

impl From<LoggerPermission> for ic_log::did::LoggerPermission {
    fn from(value: LoggerPermission) -> Self {
        match value {
            LoggerPermission::Read => ic_log::did::LoggerPermission::Read,
            LoggerPermission::Configure => ic_log::did::LoggerPermission::Configure,
        }
    }
}

/// The settings for the log canister
#[derive(Parser, Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogCanisterSettings {
    #[arg(long)]
    /// Display logs in the console
    pub enable_console: bool,
    #[arg(long)]
    /// The number of records to keep in memory
    pub in_memory_records: Option<usize>,
    #[arg(long)]
    /// The maximum length of a record
    pub max_record_length: Option<usize>,
    #[arg(
        long,
        help = "The filter to apply to the logs
        (e.g. 'debug, rune_bridge=debug')"
    )]
    /// The filter to apply to the logs
    pub log_filter: Option<String>,
    #[arg(long, value_parser = parse_acl_entries, help = "(principal,permission), (principal,permission), ...")]
    /// The ACL for the log canister
    pub acl: Option<HashSet<(Principal, LoggerPermission)>>,
}

/// Parses a string of ACL entries into a set of ACL entries.
fn parse_acl_entries(s: &str) -> Result<HashSet<(Principal, LoggerPermission)>, String> {
    s.split("),")
        .map(|entry| entry.trim().trim_start_matches('(').trim_end_matches(')'))
        .map(parse_acl_entry)
        .collect()
}

fn parse_acl_entry(s: &str) -> Result<(Principal, LoggerPermission), String> {
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    if parts.len() != 2 {
        return Err(
            "Invalid ACL entry format. Expected 'principal,permission' or '(principal,permission)'"
                .into(),
        );
    }

    let principal =
        Principal::from_text(parts[0]).map_err(|e| format!("Invalid principal: {}", e))?;

    let permission = match parts[1].to_lowercase().as_str() {
        "read" => LoggerPermission::Read,
        "configure" => LoggerPermission::Configure,
        _ => return Err("Invalid permission. Expected 'read' or 'configure'".into()),
    };

    Ok((principal, permission))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_acl_entry_valid_read() {
        let input = "2vxsx-fae,read";
        let result = parse_acl_entries(input);
        assert!(result.is_ok());
        let (principal, permission) = result.unwrap().into_iter().next().unwrap();
        assert_eq!(principal, Principal::from_text("2vxsx-fae").unwrap());
        assert_eq!(permission, LoggerPermission::Read);
    }

    #[test]
    fn test_parse_acl_entry_with_mutiple_acls() {
        let input = "(2vxsx-fae,read), (2vxsx-fae,configure)";
        let result = parse_acl_entries(input);

        assert!(result.is_ok());
        let acls = result.unwrap();
        assert_eq!(acls.len(), 2);
        assert!(acls.contains(&(
            Principal::from_text("2vxsx-fae").unwrap(),
            LoggerPermission::Read
        )));
        assert!(acls.contains(&(
            Principal::from_text("2vxsx-fae").unwrap(),
            LoggerPermission::Configure
        )));
    }

    #[test]
    fn test_parse_acl_entry_valid_configure() {
        let input = "2vxsx-fae,configure";
        let result = parse_acl_entries(input);
        assert!(result.is_ok());
        let (principal, permission) = result.unwrap().into_iter().next().unwrap();
        assert_eq!(principal, Principal::from_text("2vxsx-fae").unwrap());
        assert_eq!(permission, LoggerPermission::Configure);
    }

    #[test]
    fn test_parse_acl_entry_invalid_format() {
        let input = "2vxsx-fae";
        let result = parse_acl_entries(input);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Invalid ACL entry format. Expected 'principal,permission' or '(principal,permission)'"
        );
    }

    #[test]
    fn test_parse_acl_entry_invalid_principal() {
        let input = "invalid-principal,read";
        let result = parse_acl_entries(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().starts_with("Invalid principal:"));
    }

    #[test]
    fn test_parse_acl_entry_invalid_permission() {
        let input = "2vxsx-fae,write";
        let result = parse_acl_entries(input);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Invalid permission. Expected 'read' or 'configure'"
        );
    }

    #[test]
    fn test_parse_acl_entry_case_insensitive_permission() {
        let input = "2vxsx-fae,READ";
        let result = parse_acl_entries(input);
        assert!(result.is_ok());
        let (_, permission) = result.unwrap().into_iter().next().unwrap();
        assert_eq!(permission, LoggerPermission::Read);
    }

    #[test]
    fn test_parse_acl_entry_trimmed_input() {
        let input = " 2vxsx-fae , configure ";
        let result = parse_acl_entries(input);
        assert!(result.is_ok());
        let (principal, permission) = result.unwrap().into_iter().next().unwrap();
        assert_eq!(principal, Principal::from_text("2vxsx-fae").unwrap());
        assert_eq!(permission, LoggerPermission::Configure);
    }
}
