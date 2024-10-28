use candid::CandidType;
use eth_signer::sign_strategy::SigningStrategy;
use serde::Deserialize;

use crate::evm_link::EvmLink;

#[derive(Debug, Clone, Deserialize, CandidType)]
pub struct BaseEvmSettings {
    pub evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
}
