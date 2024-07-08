use std::cell::RefCell;
use std::rc::Rc;

use bridge_did::error::Result;
use bridge_did::init::BridgeInitData;
use bridge_utils::evm_bridge::EvmParams;
use candid::Principal;
use eth_signer::sign_strategy::TransactionSigner;
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ic_canister_client::IcCanisterClient;
use ic_exports::ic_kit::ic;
use ic_storage::IcStorage;

use crate::config::BridgeConfig;
use crate::signer::SignerStorage;

#[derive(Debug, Clone, Default)]
pub struct BridgeCore {
    pub config: BridgeConfig,
}

impl BridgeCore {
    pub fn init(&mut self, init_data: &BridgeInitData) {
        self.inspect_new_owner_is_valid(init_data.owner);
        self.config = BridgeConfig::init(init_data);
        self.init_signer(0)
            .expect("Failed to initialize transaction signer.");
    }

    pub fn reload(&mut self) {
        self.config = BridgeConfig::load();
    }

    pub fn init_signer(&mut self, chain_id: u32) -> Result<()> {
        SignerStorage {}.reset(self.config.get_signing_strategy().clone(), chain_id)
    }

    pub fn get_transaction_signer(&self) -> impl TransactionSigner {
        SignerStorage {}.get_transaction_signer()
    }

    /// Returns EVM client
    pub fn get_evm_client(&self) -> EthJsonRpcClient<impl Client> {
        EthJsonRpcClient::new(IcCanisterClient::new(self.config.get_evm_principal()))
    }

    /// Returns parameters of EVM canister with which the minter canister works.
    pub fn get_evm_params(&self) -> Option<EvmParams> {
        self.config.get_evm_params()
    }

    /// Updates parameters of EVM canister with which the minter canister works.
    pub fn update_evm_params<F: FnOnce(&mut EvmParams)>(&mut self, f: F) {
        let need_to_update_signer = self.config.get_evm_params().is_none();
        self.config.update_evm_params(f);

        if need_to_update_signer {
            if let Some(EvmParams { chain_id, .. }) = self.config.get_evm_params() {
                if let Err(err) = self.init_signer(chain_id) {
                    log::error!("Failed to initialize signer: {err:?}");
                }
            }
        }
    }

    pub fn set_owner(&mut self, new_owner: Principal) {
        self.inspect_set_owner();
        self.inspect_new_owner_is_valid(new_owner);
        self.config.set_owner(new_owner);
    }

    fn inspect_new_owner_is_valid(&self, new_owner: Principal) {
        if new_owner == Principal::anonymous() {
            ic::trap("Owner cannot be an anonymous");
        }
    }

    pub fn inspect_ic_logs(&self) {
        self.inspect_caller_is_owner()
    }

    pub fn inspect_set_logger_filter(&self) {
        self.inspect_caller_is_owner()
    }

    pub fn inspect_set_owner(&self) {
        self.inspect_caller_is_owner()
    }

    pub fn inspect_set_evm_principal(&self) {
        self.inspect_caller_is_owner()
    }

    pub fn inspect_set_bft_bridge_contract(&self) {
        self.inspect_caller_is_owner()
    }

    fn inspect_caller_is_owner(&self) {
        let owner = self.config.get_owner();
        let caller = ic::caller();
        if ic::caller() != self.config.get_owner() {
            log::debug!(
                "Owner only method is called by non-owner. Owner: {owner}. Caller: {caller}"
            );
            ic::trap("Running this method is only allowed for the owner of the canister")
        }
    }
}

impl IcStorage for BridgeCore {
    fn get() -> Rc<RefCell<Self>> {
        CORE.with(|cell| cell.clone())
    }
}

thread_local! {
    pub static CORE: Rc<RefCell<BridgeCore>> = Rc::default();
}
