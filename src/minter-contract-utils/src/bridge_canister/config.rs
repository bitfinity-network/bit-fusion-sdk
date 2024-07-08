use std::borrow::Cow;
use std::cell::RefCell;

use bridge_did::init::BridgeInitData;
use candid::{CandidType, Deserialize, Principal};
use did::{codec, H160};
use eth_signer::sign_strategy::{SigningKeyId, SigningStrategy};
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ic_canister_client::IcCanisterClient;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, Storable, VirtualMemory};

use crate::bridge_canister::memory::{CONFIG_MEMORY_ID, MEMORY_MANAGER};
use crate::evm_bridge::EvmParams;

#[derive(Debug, Clone, Deserialize, CandidType, PartialEq, Eq, serde::Serialize)]
pub struct BridgeConfig {
    owner: Principal,
    evm_principal: Principal,
    evm_params: Option<EvmParams>,
    bft_bridge_contract_address: Option<H160>,
    signing_strategy: SigningStrategy,
}

impl BridgeConfig {
    pub fn init(init_data: &BridgeInitData) -> Self {
        let value = BridgeConfig {
            owner: init_data.owner,
            evm_principal: init_data.evm_principal,
            evm_params: None,
            bft_bridge_contract_address: None,
            signing_strategy: init_data.signing_strategy.clone(),
        };

        value.clone().store();

        value
    }

    pub fn load() -> Self {
        CONFIG_CELL.with(|cell| cell.borrow().get().clone())
    }

    /// Returns principal of canister owner.
    pub fn get_owner(&self) -> Principal {
        self.owner
    }

    /// Sets a new principal for canister owner.
    pub fn set_owner(&mut self, owner: Principal) {
        self.owner = owner;
        self.clone().store()
    }

    /// Returns principal of EVM canister with which the minter canister works.
    pub fn get_evm_principal(&self) -> Principal {
        self.evm_principal
    }

    /// Sets principal of EVM canister with which the minter canister works.
    pub fn set_evm_principal(&mut self, evm: Principal) {
        self.evm_principal = evm;
        self.clone().store();
    }

    /// Returns parameters of EVM canister with which the minter canister works.
    pub(crate) fn get_evm_params(&self) -> Option<EvmParams> {
        self.evm_params.clone()
    }

    /// Updates parameters of EVM canister with which the minter canister works.
    pub(crate) fn update_evm_params<F: FnOnce(&mut EvmParams)>(&mut self, f: F) {
        let mut updated = self.evm_params.clone().unwrap_or_default();
        f(&mut updated);
        self.evm_params = Some(updated);
        self.clone().store();
    }

    pub fn get_signing_strategy(&self) -> &SigningStrategy {
        &self.signing_strategy
    }

    /// Returns EVM client
    pub fn get_evm_client(&self) -> EthJsonRpcClient<impl Client> {
        EthJsonRpcClient::new(IcCanisterClient::new(self.get_evm_principal()))
    }

    pub fn get_bft_bridge_contract(&self) -> Option<H160> {
        self.bft_bridge_contract_address.clone()
    }

    pub fn set_bft_bridge_contract(&mut self, address: Option<H160>) {
        self.bft_bridge_contract_address = address;
        self.clone().store();
    }

    fn store(self) {
        CONFIG_CELL
            .with(|cell| cell.borrow_mut().set(self))
            .expect("failed to update config stable memory data")
    }
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            owner: Principal::management_canister(),
            evm_principal: Principal::management_canister(),
            evm_params: None,
            bft_bridge_contract_address: None,
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: SigningKeyId::Test,
            },
        }
    }
}

impl Storable for BridgeConfig {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        codec::encode(&self).into()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        codec::decode(bytes.as_ref())
    }

    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Unbounded;
}

thread_local! {
    static CONFIG_CELL: RefCell<StableCell<BridgeConfig, VirtualMemory<DefaultMemoryImpl>>> = {
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(CONFIG_MEMORY_ID)), BridgeConfig::default())
            .expect("stable memory config initialization failed"))
    };
}
