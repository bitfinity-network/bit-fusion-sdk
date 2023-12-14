use std::borrow::Cow;
use std::fmt;

use candid::{CandidType, Principal};
use did::codec;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, Storable, VirtualMemory};
use serde::{Deserialize, Serialize};

use crate::memory::{CONFIG_MEMORY_ID, MEMORY_MANAGER};

pub struct Config {
    data: StableCell<ConfigData, VirtualMemory<DefaultMemoryImpl>>,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("data", &self.data.get())
            .finish()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data: StableCell::new(
                MEMORY_MANAGER.with(|mm| mm.get(CONFIG_MEMORY_ID)),
                ConfigData::default(),
            )
            .expect("stable memory config initialization failed"),
        }
    }
}

impl Config {
    pub fn init(config: ConfigData) {
        Self::default().update_data(|data| {
            *data = config;
        })
    }

    pub fn get_evmc_principal(&self) -> Principal {
        self.data.get().evmc_principal
    }

    pub fn set_evmc_principal(&mut self, principal: Principal) {
        self.update_data(|data| data.evmc_principal = principal);
    }

    pub fn get_external_evm_link(&self) -> EvmLink {
        self.data.get().external_evm_link.clone()
    }

    pub fn set_external_evm_link(&mut self, external_evm: EvmLink) {
        self.update_data(|data| data.external_evm_link = external_evm);
    }

    fn update_data<F>(&mut self, f: F)
    where
        F: FnOnce(&mut ConfigData),
    {
            let mut data = self.data.get().clone();
            f(&mut data);
            self.data.set(data)
                .expect("failed to update config stable memory data");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct ConfigData {
    pub evmc_principal: Principal,
    pub external_evm_link: EvmLink,
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            evmc_principal: Principal::anonymous(),
            external_evm_link: EvmLink::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum EvmLink {
    Http(String),
    Ic(Principal),
}

impl Default for EvmLink {
    fn default() -> Self {
        EvmLink::Ic(Principal::anonymous())
    }
}

impl Storable for ConfigData {
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
    use super::*;
    use candid::Principal;
    use did::codec;
    use ic_stable_structures::Storable;

    #[test]
    fn test_to_bytes() {
        let config_data = ConfigData::default();
        let bytes = config_data.to_bytes();
        let decoded_config_data = ConfigData::from_bytes(bytes.clone());
        assert_eq!(config_data, decoded_config_data);
    }

    #[test]
    fn test_from_bytes() {
        let config_data = ConfigData::default();
        let bytes = codec::encode(&config_data).into();
        let decoded_config_data = ConfigData::from_bytes(bytes);
        assert_eq!(config_data, decoded_config_data);
    }

    #[test]
    fn test_config_getters_and_setters() {
        let mut config = Config::default();

        assert_eq!(config.get_evmc_principal(), Principal::anonymous());
        assert_eq!(config.get_external_evm_link(), EvmLink::default());

        let evmc_principal = Principal::from_slice(b"evmc");
        let external_evm = EvmLink::Http("https://example.com".to_string());

        config.set_evmc_principal(evmc_principal);
        config.set_external_evm_link(external_evm.clone());

        assert_eq!(config.get_evmc_principal(), evmc_principal);
        assert_eq!(config.get_external_evm_link(), external_evm);
    }
}
