mod erc;
mod icrc;
mod rune;

use std::collections::HashSet;

use candid::{CandidType, Principal};
use clap::{Parser, ValueEnum};
use eth_signer::sign_strategy;
use serde::{Deserialize, Serialize};

pub use erc::*;
pub use icrc::*;
pub use rune::*;

#[derive(ValueEnum, Debug, Serialize, Deserialize, Clone, CandidType, PartialEq, Eq)]
pub enum SigningKeyId {
    Dfx,
    Test,
    Production,
}

impl From<SigningKeyId> for sign_strategy::SigningKeyId {
    fn from(value: SigningKeyId) -> Self {
        match value {
            SigningKeyId::Dfx => todo!(),
            SigningKeyId::Test => todo!(),
            SigningKeyId::Production => todo!(),
        }
    }
}

#[derive(
    ValueEnum, Debug, Clone, Copy, Serialize, CandidType, Deserialize, Eq, PartialEq, Hash,
)]
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

#[derive(Parser, Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogCanisterSettings {
    #[arg(long)]
    pub enable_console: Option<bool>,
    #[arg(long)]
    pub in_memory_records: Option<usize>,
    #[arg(long)]
    pub max_record_length: Option<usize>,
    #[arg(long)]
    pub log_filter: Option<String>,
    #[arg(long, value_parser = parse_acl_entry)]
    pub acl: Option<HashSet<(Principal, LoggerPermission)>>,
}

fn parse_acl_entry(s: &str) -> Result<(Principal, LoggerPermission), String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err("Invalid ACL entry format. Expected 'principal,permission'".to_string());
    }

    let principal =
        Principal::from_text(parts[0].trim()).map_err(|e| format!("Invalid principal: {}", e))?;

    let permission = match parts[1].trim().to_lowercase().as_str() {
        "read" => LoggerPermission::Read,
        "configure" => LoggerPermission::Configure,
        _ => return Err("Invalid permission. Expected 'read' or 'configure'".to_string()),
    };

    Ok((principal, permission))
}
