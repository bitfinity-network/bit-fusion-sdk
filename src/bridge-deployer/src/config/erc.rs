use clap::Parser;
use serde::{Deserialize, Serialize};

use super::SigningKeyId;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct BaseEvmSettingsConfig {
    #[arg(long)]
    pub singing_key_id: SigningKeyId,
}
