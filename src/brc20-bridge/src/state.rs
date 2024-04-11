use candid::{CandidType, Principal};
use did::H160;
use eth_signer::sign_strategy::{SigningStrategy, TxSigner};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_log::{init_log, LogSettings};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableCell, VirtualMemory};
use minter_contract_utils::evm_bridge::{EvmInfo, EvmParams};
use minter_contract_utils::evm_link::EvmLink;
use serde::Deserialize;

use crate::constant::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};
use crate::memory::{MEMORY_MANAGER, SIGNER_MEMORY_ID};
use crate::store::{BurnRequestStore, MintOrdersStore};

type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

pub struct State {
    config: Brc20BridgeConfig,
    bft_config: BftBridgeConfig,
    signer: SignerStorage,
    mint_orders: MintOrdersStore,
    burn_requests: BurnRequestStore,
    evm_params: Option<EvmParams>,
}

#[derive(Debug, CandidType, Deserialize)]
pub struct Brc20BridgeConfig {
    pub inscriber: Principal,
    pub network: BitcoinNetwork,
    pub evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
    pub admin: Principal,
    pub inscriber_fee: u64,
    pub logger: LogSettings,
}

impl Default for Brc20BridgeConfig {
    fn default() -> Self {
        Self {
            inscriber: Principal::anonymous(),
            network: BitcoinNetwork::Regtest,
            evm_link: EvmLink::default(),
            signing_strategy: SigningStrategy::Local {
                private_key: [0; 32],
            },
            admin: Principal::management_canister(),
            inscriber_fee: 10,
            logger: LogSettings::default(),
        }
    }
}

#[derive(Default, Debug, CandidType, Deserialize)]
pub struct BftBridgeConfig {
    pub erc20_chain_id: u32,
    pub bridge_address: H160,
    pub token_address: H160,
    pub token_name: [u8; 32],
    pub token_symbol: [u8; 16],
    pub decimals: u8,
}

impl Default for State {
    fn default() -> Self {
        let default_signer = SigningStrategy::Local {
            private_key: [1; 32],
        }
        .make_signer(0)
        .expect("Failed to create default signer");

        let signer = SignerStorage::new(
            MEMORY_MANAGER.with(|mm| mm.get(SIGNER_MEMORY_ID)),
            default_signer,
        )
        .expect("failed to initialize transaction signer");

        Self {
            config: Default::default(),
            bft_config: Default::default(),
            signer,
            mint_orders: Default::default(),
            burn_requests: Default::default(),
            evm_params: None,
        }
    }
}

impl State {
    pub fn configure(&mut self, config: Brc20BridgeConfig) {
        let signer = config
            .signing_strategy
            .clone()
            .make_signer(0)
            .expect("Failed to create signer");
        let stable = SignerStorage::new(MEMORY_MANAGER.with(|mm| mm.get(SIGNER_MEMORY_ID)), signer)
            .expect("failed to init signer in stable memory");
        self.signer = stable;

        init_log(&config.logger).expect("failed to init logger");

        self.config = config;
    }

    pub fn configure_bft(&mut self, bft_config: BftBridgeConfig) {
        self.bft_config = bft_config;
    }

    pub fn inscriber(&self) -> Principal {
        self.config.inscriber
    }

    pub fn erc20_chain_id(&self) -> u32 {
        self.bft_config.erc20_chain_id
    }

    pub fn btc_chain_id(&self) -> u32 {
        match self.config.network {
            BitcoinNetwork::Mainnet => MAINNET_CHAIN_ID,
            BitcoinNetwork::Testnet => TESTNET_CHAIN_ID,
            BitcoinNetwork::Regtest => REGTEST_CHAIN_ID,
        }
    }

    pub fn signer(&self) -> &SignerStorage {
        &self.signer
    }

    pub fn mint_orders(&self) -> &MintOrdersStore {
        &self.mint_orders
    }

    pub fn mint_orders_mut(&mut self) -> &mut MintOrdersStore {
        &mut self.mint_orders
    }

    pub fn burn_request_store(&self) -> &BurnRequestStore {
        &self.burn_requests
    }

    pub fn burn_request_store_mut(&mut self) -> &mut BurnRequestStore {
        &mut self.burn_requests
    }

    pub fn get_evm_info(&self) -> EvmInfo {
        EvmInfo {
            link: self.config.evm_link.clone(),
            bridge_contract: self.bft_config.bridge_address.clone(),
            params: self.evm_params.clone(),
        }
    }

    pub fn get_evm_params(&self) -> &Option<EvmParams> {
        &self.evm_params
    }

    pub fn token_address(&self) -> &H160 {
        &self.bft_config.token_address
    }

    pub fn token_name(&self) -> [u8; 32] {
        self.bft_config.token_name
    }

    pub fn token_symbol(&self) -> [u8; 16] {
        self.bft_config.token_symbol
    }

    pub fn decimals(&self) -> u8 {
        self.bft_config.decimals
    }

    pub fn update_evm_params(&mut self, f: impl FnOnce(&mut Option<EvmParams>)) {
        f(&mut self.evm_params)
    }

    pub fn admin(&self) -> Principal {
        self.config.admin
    }

    pub fn check_admin(&self, caller: Principal) {
        if caller != self.admin() {
            panic!("access denied");
        }
    }

    pub fn inscriber_fee(&self) -> u64 {
        self.config.inscriber_fee
    }
}
