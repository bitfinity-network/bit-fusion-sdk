use candid::{CandidType, Principal};
use clap::Parser;
use serde::{Deserialize, Serialize};

use super::SigningKeyId;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct BaseEvmSettingsConfig {
    #[arg(long)]
    pub evm_link: Principal,
    #[arg(long)]
    pub singing_key_id: SigningKeyId,
}

#[derive(CandidType, Debug, Serialize, Deserialize, Clone)]
pub enum EvmLink {
    Ic(Principal),
}
