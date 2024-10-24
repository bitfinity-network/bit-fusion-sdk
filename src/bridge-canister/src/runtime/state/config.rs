use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use bridge_did::error::{BftResult, Error};
use bridge_did::evm_link::EvmLink;
use bridge_did::init::BridgeInitData;
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::evm_link::EvmLinkClient;
use bridge_utils::query::{
    self, Query, QueryType, CHAINID_ID, GAS_PRICE_ID, LATEST_BLOCK_ID, NONCE_ID,
};
use candid::{CandidType, Principal};
use did::{codec, H160, U256};
use eth_signer::sign_strategy::{SigningStrategy, TransactionSigner};
use ic_stable_structures::{CellStructure, StableCell, Storable};
use jsonrpc_core::Id;
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
        match init_data.evm_link {
            EvmLink::Ic(principal) if principal == Principal::anonymous() => {
                log::error!("unexpected anonymous evm principal");
                panic!("unexpected anonymous evm principal");
            }
            EvmLink::Ic(principal) if principal == Principal::management_canister() => {
                log::error!("unexpected management canister as evm principal");
                panic!("unexpected management canister as evm principal");
            }
            _ => {}
        }

        let new_config = Config {
            owner: init_data.owner,
            evm_link: init_data.evm_link.clone(),
            evm_params: None,
            bft_bridge_contract_address: None,
            signing_strategy: init_data.signing_strategy.clone(),
        };

        self.update(|stored| *stored = new_config);
    }

    /// Query EVM params using the EvmLink in the config data.
    pub async fn init_evm_params(config: Rc<RefCell<Self>>) -> BftResult<()> {
        log::trace!("initializing evm params");

        let link = config.borrow().get_evm_link();
        let client = link.get_json_rpc_client();
        let responses = query::batch_query(
            &client,
            &[
                QueryType::GasPrice,
                QueryType::ChainID,
                QueryType::LatestBlock,
            ],
        )
        .await
        .map_err(|e| Error::EvmRequestFailed(format!("failed to query evm params: {e}")))?;

        log::trace!("initializing evm params responses: {responses:?}");

        let gas_price: U256 = responses
            .get_value_by_id(Id::Str(GAS_PRICE_ID.into()))
            .map_err(|e| Error::EvmRequestFailed(format!("failed to query gas price: {e}")))?;
        let chain_id: U256 = responses
            .get_value_by_id(Id::Str(CHAINID_ID.into()))
            .map_err(|e| Error::EvmRequestFailed(format!("failed to query chain id: {e}")))?;
        let latest_block: U256 = responses
            .get_value_by_id(Id::Str(LATEST_BLOCK_ID.into()))
            .map_err(|e| Error::EvmRequestFailed(format!("failed to query latest block: {e}")))?;

        let params = EvmParams {
            nonce: 0,
            gas_price,
            chain_id: chain_id.0.as_u32(),
            next_block: latest_block.0.as_u64(),
        };

        config
            .borrow_mut()
            .update_evm_params(|p| *p = params.clone());

        log::trace!("evm params initialized: {params:?}");

        Ok(())
    }

    /// Updates evm params in the given config, using the EvmLink from there.
    pub async fn refresh_evm_params(config: Rc<RefCell<Self>>) -> BftResult<()> {
        log::trace!("updating evm params");

        let link = config.borrow().get_evm_link();
        let client = link.get_json_rpc_client();
        if config.borrow().get_evm_params().is_err() {
            ConfigStorage::init_evm_params(config.clone()).await?;
        };

        let address = {
            let signer = config.borrow().get_signer()?;
            signer.get_address().await?
        };

        let responses = query::batch_query(
            &client,
            &[
                QueryType::Nonce {
                    address: address.into(),
                },
                QueryType::GasPrice,
            ],
        )
        .await
        .map_err(|e| Error::EvmRequestFailed(format!("failed to query evm params: {e}")))?;

        let nonce: U256 = responses
            .get_value_by_id(Id::Str(NONCE_ID.into()))
            .map_err(|e| Error::EvmRequestFailed(format!("failed to query nonce: {e}")))?;
        let gas_price: U256 = responses
            .get_value_by_id(Id::Str(GAS_PRICE_ID.into()))
            .map_err(|e| Error::EvmRequestFailed(format!("failed to query gas price: {e}")))?;

        config.borrow_mut().update_evm_params(|p| {
            p.nonce = nonce.0.as_u64();
            p.gas_price = gas_price;
        });

        log::trace!("evm params updated: {:?}", config.borrow().get_evm_params());

        Ok(())
    }

    /// Sets owner principal.
    pub fn set_owner(&mut self, new_owner: Principal) {
        self.update(|config| config.owner = new_owner);
    }

    /// Returns owner principal.
    pub fn get_owner(&self) -> Principal {
        self.0.get().owner
    }

    /// Checks if the caller is owner.
    pub fn check_owner(&self, caller: Principal) -> BftResult<()> {
        if caller != self.get_owner() {
            return Err(Error::AccessDenied);
        }

        Ok(())
    }

    /// Returns parameters of EVM canister with which the bridge canister works.
    pub fn get_evm_params(&self) -> BftResult<EvmParams> {
        self.0.get().evm_params.clone().ok_or_else(|| {
            Error::Initialization("failed to get uninitialized get evm params".into())
        })
    }

    /// Updates parameters of EVM canister with which the bridge canister works.
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
