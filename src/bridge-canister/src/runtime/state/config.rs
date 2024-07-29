use std::borrow::Cow;

use bridge_did::error::{BftResult, Error};
use bridge_did::init::BridgeInitData;
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::evm_link::EvmLink;
use candid::{CandidType, Principal};
use did::{codec, H160};
use eth_signer::sign_strategy::{SigningStrategy, TransactionSigner};
use ic_stable_structures::{CellStructure, StableCell, Storable};
use serde::{Deserialize, Serialize};

use crate::memory::StableMemory;

/// Stores configuration to work with EVM.
pub struct ConfigStorage(StableCell<Config, StableMemory>);

impl ConfigStorage {
    /// Stores a new SignerInfo in the given memory.
    pub fn default(memory: StableMemory) -> Self {
        let cell =
            StableCell::new(memory, Config::default()).expect("failed to initialize evm config");

        Self(cell)
    }

    /// Creates a new instance of config struct and stores it in the stable memory.
    pub fn init(&mut self, init_data: &BridgeInitData) {
        if init_data.evm_principal == Principal::anonymous() {
            log::error!("unexpected anonymous evm principal");
            panic!("unexpected anonymous evm principal");
        }

        if init_data.evm_principal == Principal::management_canister() {
            log::error!("unexpected management canister as evm principal");
            panic!("unexpected management canister as evm principal");
        }

        let evm_link = EvmLink::Ic(init_data.evm_principal);
        let new_config = Config {
            owner: init_data.owner,
            evm_link,
            evm_params: None,
            bft_bridge_contract_address: None,
            signing_strategy: init_data.signing_strategy.clone(),
        };

        self.update(|stored| *stored = new_config);
    }

    /// Sets owner principal.
    pub fn set_owner(&mut self, new_owner: Principal) {
        self.update(|config| config.owner = new_owner);
    }

    /// Returns owner principal.
    pub fn get_owner(&self) -> Principal {
        self.0.get().owner
    }

    /// Returns parameters of EVM canister with which the minter canister works.
    pub fn get_evm_params(&self) -> BftResult<EvmParams> {
        self.0.get().evm_params.clone().ok_or_else(|| {
            Error::Initialization("failed to get uninitialized get evm params".into())
        })
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

    /// Creates a signer according to `Self::signing_strategy`.
    pub fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        let config = self.0.get();
        let chain_id = self.get_evm_params()?.chain_id;
        config
            .signing_strategy
            .clone()
            .make_signer(chain_id as _)
            .map_err(|e| Error::Signing(e.to_string()))
    }

    /// Updates signing strategy.
    pub fn set_signing_strategy(&mut self, strategy: SigningStrategy) {
        self.update(|config| config.signing_strategy = strategy);
    }

    /// Returns signing strategy.
    pub fn get_signing_strategy(&self) -> SigningStrategy {
        self.0.get().signing_strategy.clone()
    }

    /// Updates config data.
    pub fn update(&mut self, f: impl FnOnce(&mut Config)) {
        let mut config = self.0.get().clone();
        f(&mut config);
        self.0.set(config).expect("failed to update config");
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct Config {
    pub owner: Principal,
    pub evm_link: EvmLink,
    pub evm_params: Option<EvmParams>,
    pub bft_bridge_contract_address: Option<H160>,
    pub signing_strategy: SigningStrategy,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            owner: Principal::management_canister(),
            evm_link: EvmLink::Ic(Principal::anonymous()),
            evm_params: None,
            bft_bridge_contract_address: None,
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: eth_signer::ic_sign::SigningKeyId::Test,
            },
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
