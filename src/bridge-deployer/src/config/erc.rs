use bridge_utils::evm_link::EvmLink;
use candid::Principal;
use clap::Parser;
use erc20_bridge::state::BaseEvmSettings;
use eth_signer::sign_strategy::SigningStrategy;
use serde::{Deserialize, Serialize};

use super::SigningKeyId;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct BaseEvmSettingsConfig {
    #[arg(long)]
    pub evm_link: Principal,
    #[arg(long)]
    pub singing_key_id: SigningKeyId,
}

impl From<BaseEvmSettingsConfig> for BaseEvmSettings {
    fn from(value: BaseEvmSettingsConfig) -> Self {
        BaseEvmSettings {
            evm_link: EvmLink::Ic(value.evm_link),
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: value.singing_key_id.into(),
            },
        }
    }
}
