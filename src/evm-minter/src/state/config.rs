use std::borrow::Cow;
use std::fmt;

use candid::{CandidType, Principal};
use did::{codec, H160};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, Storable, VirtualMemory};
use serde::{Deserialize, Serialize};

use super::Settings;
use crate::client::EvmLink;
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
    pub fn init(&mut self, settings: Settings) {
        Self::default().update_data(|data| {
            data.evms[BridgeSide::Base as usize].link = settings.base_evm_link;
            data.evms[BridgeSide::Wrapped as usize].link = settings.wrapped_evm_link;
        })
    }

    pub fn get_evm_info(&self, bridge_side: BridgeSide) -> EvmInfo {
        self.data.get().evms[bridge_side as usize].clone()
    }

    pub fn set_evm_chain_id(&mut self, chain_id: u64, bridge_side: BridgeSide) {
        self.update_data(|data| data.evms[bridge_side as usize].chain_id = Some(chain_id));
    }

    pub fn set_evm_next_block(&mut self, next_block: u64, bridge_side: BridgeSide) {
        self.update_data(|data| data.evms[bridge_side as usize].next_block = Some(next_block));
    }

    fn update_data<F>(&mut self, f: F)
    where
        F: FnOnce(&mut ConfigData),
    {
        let mut data = self.data.get().clone();
        f(&mut data);
        self.data
            .set(data)
            .expect("failed to update config stable memory data");
    }

    pub fn get_initialized_evm_info(&self, bridge_side: BridgeSide) -> Option<InitializedEvmInfo> {
        let side_idx = bridge_side as usize;

        let chain_id = self.data.get().evms[side_idx].chain_id?;
        let next_block = self.data.get().evms[side_idx].next_block?;

        Some(InitializedEvmInfo {
            link: self.data.get().evms[side_idx].link.clone(),
            bridge_contract: self.data.get().evms[side_idx].bridge_contract.clone(),
            chain_id,
            next_block,
        })
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum BridgeSide {
    Base = 0,
    Wrapped = 1,
}

impl BridgeSide {
    pub fn other(self) -> Self {
        match self {
            Self::Base => Self::Wrapped,
            Self::Wrapped => Self::Base,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct EvmInfo {
    pub link: EvmLink,
    pub bridge_contract: H160,
    pub chain_id: Option<u64>,
    pub next_block: Option<u64>,
}

pub struct InitializedEvmInfo {
    pub link: EvmLink,
    pub bridge_contract: H160,
    pub chain_id: u64,
    pub next_block: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct ConfigData {
    pub admin: Principal,
    pub evms: [EvmInfo; 2],
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            admin: Principal::anonymous(),
            evms: Default::default(),
        }
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
    use did::codec;
    use ic_stable_structures::Storable;

    use super::*;

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

        assert_eq!(config.get_evm_info(BridgeSide::Base), EvmInfo::default());
        assert_eq!(config.get_evm_info(BridgeSide::Wrapped), EvmInfo::default());

        let base_chain_id = 42;
        let wrapped_chain_id = 84;
        config.set_evm_chain_id(base_chain_id, BridgeSide::Base);
        config.set_evm_chain_id(wrapped_chain_id, BridgeSide::Wrapped);

        assert_eq!(
            config.get_evm_info(BridgeSide::Base).chain_id,
            Some(base_chain_id)
        );
        assert_eq!(
            config.get_evm_info(BridgeSide::Wrapped).chain_id,
            Some(wrapped_chain_id)
        );

        let base_next_block = 1;
        let wrapped_next_block = 2;

        config.set_evm_next_block(1, BridgeSide::Base);
        config.set_evm_next_block(2, BridgeSide::Wrapped);

        assert_eq!(
            config.get_evm_info(BridgeSide::Base).next_block,
            Some(base_next_block)
        );
        assert_eq!(
            config.get_evm_info(BridgeSide::Wrapped).next_block,
            Some(wrapped_next_block)
        );
    }
}
