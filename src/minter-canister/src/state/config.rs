use std::borrow::Cow;
use std::cell::RefCell;

use candid::{CandidType, Deserialize, Principal};
use did::{codec, H160, U256};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, Storable, VirtualMemory};

use super::Settings;
use crate::constant::{CONFIG_MEMORY_ID, DEFAULT_CHAIN_ID, DEFAULT_GAS_PRICE};
use crate::memory::MEMORY_MANAGER;

/// Minter canister configuration.
#[derive(Default, Clone)]
pub struct Config {}

impl Config {
    /// Clear configuration and initialize it with data from `settings`.
    pub fn reset(&mut self, settings: Settings) {
        let new_data = ConfigData {
            owner: settings.owner,
            evm_principal: settings.evm_principal,
            evm_chain_id: settings.chain_id,
            bft_bridge_contract: settings.bft_bridge_contract,
            evm_gas_price: settings.evm_gas_price,
            spender_principal: settings.spender_principal,
        };

        self.update_data(|data| *data = new_data);
    }

    /// Returns principal of canister owner.
    pub fn get_owner(&self) -> Principal {
        self.with_data(|data| data.get().owner)
    }

    /// Sets a new principal for canister owner.
    pub fn set_owner(&mut self, owner: Principal) {
        self.update_data(|data| data.owner = owner);
    }

    /// Returns principal of EVM canister with which the minter canister works.
    pub fn get_evm_principal(&self) -> Principal {
        self.with_data(|data| data.get().evm_principal)
    }

    /// Sets principal of EVM canister with which the minter canister works.
    pub fn set_evm_principal(&mut self, evm: Principal) {
        self.update_data(|data| data.evm_principal = evm);
    }

    /// Returns the chain ID
    pub fn get_evmc_chain_id(&self) -> u32 {
        self.with_data(|data| data.get().evm_chain_id)
    }

    /// Returns evm gas price
    pub fn get_evm_gas_price(&self) -> U256 {
        self.with_data(|data| data.get().evm_gas_price.clone())
    }

    pub fn set_evm_gas_price(&mut self, evm_gas_price: U256) {
        self.update_data(|data| data.evm_gas_price = evm_gas_price);
    }

    pub fn get_bft_bridge_contract(&self) -> Option<H160> {
        self.with_data(|data| data.get().bft_bridge_contract.clone())
    }

    pub fn set_bft_bridge_contract(&mut self, bft_bridge: did::H160) {
        self.update_data(|data| data.bft_bridge_contract = Some(bft_bridge));
    }

    fn with_data<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&StableCell<ConfigData, VirtualMemory<DefaultMemoryImpl>>) -> T,
    {
        CONFIG_CELL.with(|cell| f(&mut cell.borrow()))
    }

    fn with_mut_data<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut StableCell<ConfigData, VirtualMemory<DefaultMemoryImpl>>) -> T,
    {
        CONFIG_CELL.with(|cell| f(&mut cell.borrow_mut()))
    }

    fn update_data<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut ConfigData) -> T,
    {
        self.with_mut_data(|data| {
            let mut old_data = data.get().clone();
            let result = f(&mut old_data);
            data.set(old_data)
                .expect("failed to update config stable memory data");
            result
        })
    }

    /// Returns principal of spender canister.
    pub fn get_spender_principal(&self) -> Principal {
        self.with_data(|data| data.get().spender_principal)
    }
}

#[derive(Debug, Clone, Deserialize, CandidType, PartialEq, Eq, serde::Serialize)]
pub struct ConfigData {
    pub owner: Principal,
    pub evm_principal: Principal,
    pub evm_chain_id: u32,
    pub bft_bridge_contract: Option<H160>,

    pub evm_gas_price: U256,
    pub spender_principal: Principal,
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            owner: Principal::anonymous(),
            evm_principal: Principal::anonymous(),
            evm_chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: None,

            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            spender_principal: Principal::anonymous(),
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

thread_local! {
    static CONFIG_CELL: RefCell<StableCell<ConfigData, VirtualMemory<DefaultMemoryImpl>>> = {
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(CONFIG_MEMORY_ID)), ConfigData::default())
            .expect("stable memory config initialization failed"))
    };
}

#[cfg(test)]
mod tests {
    use candid::Principal;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_exports::ic_kit::MockContext;
    use ic_stable_structures::Storable;

    use super::*;
    use crate::constant::DEFAULT_GAS_PRICE;
    use crate::state::config::ConfigData;
    use crate::state::Settings;

    fn get_config() -> Config {
        MockContext::new().inject();
        let mut config = Config::default();
        config.reset(Settings::default());
        config
    }

    #[test]
    fn config_serialization() {
        let config = ConfigData::default();
        let encoded = config.to_bytes();
        let decoded = ConfigData::from_bytes(encoded);
        assert_eq!(config, decoded);
    }

    #[test]
    fn reset_should_update_config() {
        let mut config = get_config();

        let settings = Settings {
            owner: Principal::management_canister(),
            evm_principal: Principal::anonymous(),
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: Some(H160::from_slice(&[22; 20])),
            spender_principal: Principal::anonymous(),
            process_transactions_results_interval: None,
        };

        config.reset(settings.clone());

        assert_eq!(config.get_owner(), settings.owner);
        assert_eq!(config.get_evm_principal(), settings.evm_principal);
    }

    #[test]
    fn config_data_stored_after_set() {
        let mut config = get_config();

        config.set_owner(Principal::management_canister());
        config.set_evm_principal(Principal::management_canister());

        assert_eq!(config.get_owner(), Principal::management_canister());
        assert_eq!(config.get_evm_principal(), Principal::management_canister());
    }
}
