use candid::Principal;
use clap::Parser;
use serde::{Deserialize, Serialize};

use super::SigningKeyId;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct BaseEvmSettingsConfig {
    /// EVM canister to link to; if not provided, the default one will be used based on the network
    #[arg(long)]
    pub evm: Option<Principal>,
    #[arg(long)]
    pub singing_key_id: SigningKeyId,
}
