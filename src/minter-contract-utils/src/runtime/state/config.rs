use std::borrow::Cow;

use candid::{CandidType, Principal};
use did::{codec, H160};
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{CellStructure, StableCell, Storable};
use serde::{Deserialize, Serialize};

use crate::evm_bridge::EvmParams;
use crate::evm_link::EvmLink;

/// Stores configuration to work with EVM.
pub struct ConfigStorage<M: Memory>(StableCell<Config, M>);

impl<M: Memory> ConfigStorage<M> {
    /// Stores a new SignerInfo in the given memory.
    pub fn default(memory: M) -> Self {
        let cell =
            StableCell::new(memory, Config::default()).expect("failed to initialize evm config");

        Self(cell)
    }

    /// Reset the config data.
    pub fn reset(&mut self, config: Config) {
        self.0.set(config).expect("failed to update EVM config");
    }

    /// Returns parameters of EVM canister with which the minter canister works.
    pub fn get_evm_params(&self) -> Option<EvmParams> {
        self.0.get().evm_params.clone()
    }

    /// Updates parameters of EVM canister with which the minter canister works.
    pub fn update_evm_params<F: FnOnce(&mut EvmParams)>(&mut self, f: F) {
        self.update(|config| {
            let mut params = config.evm_params.clone().unwrap_or_default();
            f(&mut params);
            config.evm_params = Some(params);
        })
    }

    /// Sets EVM link
    pub fn set_evm_link(&mut self, link: EvmLink) {
        self.update(|config| config.evm_link = link);
    }

    /// Returns EVM link
    pub fn get_evm_link(&self) -> EvmLink {
        self.0.get().evm_link.clone()
    }

    /// Returns bridge contract address for EVM.
    pub fn get_bft_bridge_contract(&self) -> Option<H160> {
        self.0.get().bft_bridge_contract_address.clone()
    }

    /// Set bridge contract address for EVM.
    pub fn set_bft_bridge_contract(&mut self, address: H160) {
        self.update(|config| config.bft_bridge_contract_address = Some(address));
    }

    fn update(&mut self, f: impl FnOnce(&mut Config)) {
        let mut config = self.0.get().clone();
        f(&mut config);
        self.0.set(config);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct Config {
    pub evm_link: EvmLink,
    pub evm_params: Option<EvmParams>,
    pub bft_bridge_contract_address: Option<H160>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            evm_link: EvmLink::Ic(Principal::anonymous()),
            evm_params: None,
            bft_bridge_contract_address: None,
        }
    }
}

impl Storable for Config {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        codec::encode(&self).into()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        codec::decode(bytes.as_ref())
    }

    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Unbounded;
}

#[cfg(test)]
mod tests {
    use ic_stable_structures::Storable;

    use crate::runtime::state::config::Config;

    #[test]
    fn config_serialization() {
        let config = Config::default();
        let encoded = config.to_bytes();
        let decoded = Config::from_bytes(encoded);
        assert_eq!(config, decoded);
    }
}
