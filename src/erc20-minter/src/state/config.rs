use std::borrow::Cow;
use std::fmt;

use bridge_canister::memory::memory_by_id;
use bridge_utils::evm_bridge::{BridgeSide, EvmInfo, EvmParams};
use candid::{CandidType, Principal};
use did::{codec, H160};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, Storable, VirtualMemory};
use serde::{Deserialize, Serialize};

use super::Settings;
use crate::memory::CONFIG_MEMORY_ID;

/// Configuration storage for the erc20-minter canister.
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
            data: StableCell::new(memory_by_id(CONFIG_MEMORY_ID), ConfigData::default())
                .expect("stable memory config initialization failed"),
        }
    }
}

impl Config {
    /// Initializes the config.
    pub fn init(&mut self, admin: Principal, settings: Settings) {
        self.update_data(|data| {
            data.admin = admin;

            let base_evm = &mut data.evm_info_by_side_mut(BridgeSide::Base);
            base_evm.link = settings.base_evm_link;

            let wrapped_evm = &mut data.evm_info_by_side_mut(BridgeSide::Wrapped);
            wrapped_evm.link = settings.wrapped_evm_link;
        })
    }

    /// Returns evm info for the given bridge side.
    pub fn get_evm_info(&self, side: BridgeSide) -> EvmInfo {
        self.data.get().evm_info_by_side(side).clone()
    }

    /// Returns bft bridge contract for the given bridge side.
    pub fn get_bft_bridge_contract(&self, side: BridgeSide) -> Option<H160> {
        self.data.get().bridge_contract_by_side(side).clone()
    }

    /// Updates bft bridge contract for the given bridge side.
    pub fn set_bft_bridge_contract(&mut self, side: BridgeSide, contract: H160) {
        self.update_data(|data| *data.bridge_contract_by_side_mut(side) = Some(contract))
    }

    /// Sets owner principal.
    pub fn set_admin(&mut self, admin: Principal) {
        self.update_data(|data| data.admin = admin);
    }

    /// Returns evm params for the given bridge side.
    pub fn get_evm_params(&self, side: BridgeSide) -> anyhow::Result<EvmParams> {
        self.data
            .get()
            .evm_info_by_side(side)
            .params
            .clone()
            .ok_or_else(|| {
                anyhow::Error::msg(format!("EVM params not set for bridge side: {side}",))
            })
    }

    /// Updates evm params for the given bridge side.
    pub fn update_evm_params<F: FnOnce(&mut EvmParams)>(&mut self, f: F, side: BridgeSide) {
        self.update_data(|data| {
            let mut params = data
                .evm_info_by_side(side)
                .params
                .clone()
                .unwrap_or_default();
            f(&mut params);
            data.evm_info_by_side_mut(side).params = Some(params);
        })
    }

    /// Checks if the caller is the admin.
    pub fn check_admin(&self, caller: Principal) -> Option<()> {
        (self.data.get().admin == caller).then_some(())
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
}

/// Configuration data.
#[derive(Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct ConfigData {
    pub admin: Principal,
    pub base_evm: EvmInfo,
    pub wrapped_evm: EvmInfo,
    pub base_bft_bridge: Option<H160>,
    pub wrapped_bft_bridge: Option<H160>,
}

impl ConfigData {
    /// Returns evm info for the given bridge side.
    pub fn evm_info_by_side(&self, side: BridgeSide) -> &EvmInfo {
        match side {
            BridgeSide::Base => &self.base_evm,
            BridgeSide::Wrapped => &self.wrapped_evm,
        }
    }

    /// Returns mutable evm info for the given bridge side.
    pub fn evm_info_by_side_mut(&mut self, side: BridgeSide) -> &mut EvmInfo {
        match side {
            BridgeSide::Base => &mut self.base_evm,
            BridgeSide::Wrapped => &mut self.wrapped_evm,
        }
    }

    /// Returns bft bridge status for the given bridge side.
    pub fn bridge_contract_by_side(&self, side: BridgeSide) -> &Option<H160> {
        match side {
            BridgeSide::Base => &self.base_bft_bridge,
            BridgeSide::Wrapped => &self.wrapped_bft_bridge,
        }
    }

    /// Returns mutable bft bridge contract for the given bridge side.
    pub fn bridge_contract_by_side_mut(&mut self, side: BridgeSide) -> &mut Option<H160> {
        match side {
            BridgeSide::Base => &mut self.base_bft_bridge,
            BridgeSide::Wrapped => &mut self.wrapped_bft_bridge,
        }
    }
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            admin: Principal::management_canister(),
            base_evm: EvmInfo::default(),
            wrapped_evm: EvmInfo::default(),
            base_bft_bridge: Default::default(),
            wrapped_bft_bridge: Default::default(),
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
    fn test_update_params() {
        let mut config = Config::default();

        config.get_evm_params(BridgeSide::Base).unwrap_err();
        config.update_evm_params(|params| params.next_block = 100, BridgeSide::Base);
        let params = config.get_evm_params(BridgeSide::Base).unwrap();
        assert_eq!(params.next_block, 100);

        config.get_evm_params(BridgeSide::Wrapped).unwrap_err();
        config.update_evm_params(|params| params.next_block = 200, BridgeSide::Wrapped);
        let params = config.get_evm_params(BridgeSide::Wrapped).unwrap();
        assert_eq!(params.next_block, 200);
    }
}
