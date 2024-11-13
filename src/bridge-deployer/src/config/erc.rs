use bridge_did::evm_link::EvmLink;
use candid::Principal;
use clap::Args;
use serde::{Deserialize, Serialize};

#[derive(Args, Debug, Serialize, Deserialize, Clone)]
#[group(required = true, multiple = false)]
pub struct BaseEvmSettingsConfig {
    /// EVM canister to link to; if not provided, the default one will be used based on the network
    #[arg(long)]
    pub base_evm_principal: Option<Principal>,
    #[arg(long)]
    pub base_evm_url: Option<String>,
    #[arg(long)]
    pub logs_query_delay_secs: Option<u64>,
    #[arg(long)]
    pub params_query_delay_secs: Option<u64>,
}

impl From<BaseEvmSettingsConfig> for EvmLink {
    fn from(value: BaseEvmSettingsConfig) -> Self {
        if let Some(principal) = value.base_evm_principal {
            EvmLink::Ic(principal)
        } else {
            let url = value.base_evm_url.expect("evm url is not set");
            EvmLink::Http(url)
        }
    }
}
