use std::cmp::Ordering;

use bitcoin::Network;
use candid::{CandidType, Principal};
use did::H160;
use eth_signer::sign_strategy::{SigningStrategy, TxSigner};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId};
use ic_log::{init_log, LogSettings};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableCell, VirtualMemory};
use minter_contract_utils::evm_bridge::{EvmInfo, EvmParams};
use minter_contract_utils::evm_link::EvmLink;
use serde::Deserialize;

use crate::api::BridgeError;
use crate::constant::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};
use crate::memory::{MEMORY_MANAGER, SIGNER_MEMORY_ID};
use crate::store::{Brc20Store, BurnRequestStore, MintOrdersStore};

type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

pub struct State {
    config: Brc20BridgeConfig,
    bft_config: BftBridgeConfig,
    signer: SignerStorage,
    mint_orders: MintOrdersStore,
    burn_requests: BurnRequestStore,
    inscriptions: Brc20Store,
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
    pub ordinals_indexer: String,
    pub general_indexer: String,
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
            ordinals_indexer: String::new(),
            general_indexer: String::new(),
            logger: LogSettings::default(),
        }
    }
}

impl Brc20BridgeConfig {
    fn validate_indexer_urls(&self) -> Result<(), String> {
        if self.ordinals_indexer.is_empty() && self.general_indexer.is_empty() {
            return Err("Indexer URLs are empty".to_string());
        }

        if !self.ordinals_indexer.starts_with("https") && !self.general_indexer.starts_with("https")
        {
            return Err(format!(
                "Indexer URLs must be HTTPS. Given: {} and {}",
                self.ordinals_indexer, self.general_indexer
            ));
        }

        Ok(())
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
            inscriptions: Brc20Store::default(),
            evm_params: None,
        }
    }
}

impl State {
    pub fn configure(&mut self, config: Brc20BridgeConfig) {
        if let Err(err) = config.validate_indexer_urls() {
            panic!("Invalid configuration: {err}");
        }

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

    pub fn has_brc20(&self, reveal_txid: &str) -> bool {
        self.inscriptions.has_inscription(reveal_txid)
    }

    pub fn inscriber(&self) -> Principal {
        self.config.inscriber
    }

    pub fn general_indexer_url(&self) -> String {
        self.config
            .general_indexer
            .strip_suffix('/')
            .unwrap_or_else(|| &self.config.general_indexer)
            .to_string()
    }

    pub fn ordinals_indexer_url(&self) -> String {
        self.config
            .ordinals_indexer
            .strip_suffix('/')
            .unwrap_or_else(|| &self.config.ordinals_indexer)
            .to_string()
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

    pub fn btc_network(&self) -> Network {
        match self.config.network {
            BitcoinNetwork::Mainnet => Network::Bitcoin,
            BitcoinNetwork::Testnet => Network::Testnet,
            BitcoinNetwork::Regtest => Network::Regtest,
        }
    }

    pub fn ic_btc_network(&self) -> BitcoinNetwork {
        self.config.network
    }

    pub fn signer(&self) -> &SignerStorage {
        &self.signer
    }

    #[inline]
    pub(crate) fn derivation_path(&self, address: Option<H160>) -> Vec<Vec<u8>> {
        let caller_principal = ic_exports::ic_cdk::caller().as_slice().to_vec();

        match address {
            Some(address) => vec![address.0.as_bytes().to_vec()],
            None => vec![caller_principal],
        }
    }

    #[inline]
    pub(crate) fn ecdsa_key_id(&self) -> EcdsaKeyId {
        let name = match &self.config.signing_strategy {
            SigningStrategy::Local { .. } => "none".to_string(),
            SigningStrategy::ManagementCanister { key_id } => key_id.to_string(),
        };

        EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name,
        }
    }

    pub fn mint_orders(&self) -> &MintOrdersStore {
        &self.mint_orders
    }

    pub fn mint_orders_mut(&mut self) -> &mut MintOrdersStore {
        &mut self.mint_orders
    }

    pub fn burn_requests(&self) -> &BurnRequestStore {
        &self.burn_requests
    }

    pub fn burn_requests_mut(&mut self) -> &mut BurnRequestStore {
        &mut self.burn_requests
    }

    pub fn inscriptions(&self) -> &Brc20Store {
        &self.inscriptions
    }

    pub fn inscriptions_mut(&mut self) -> &mut Brc20Store {
        &mut self.inscriptions
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

    pub(crate) fn set_token_symbol(&mut self, brc20_tick: &str) -> Result<(), BridgeError> {
        let bytes = brc20_tick.as_bytes();

        match bytes.len().cmp(&16usize) {
            Ordering::Equal => {
                self.bft_config.token_symbol.copy_from_slice(bytes);
                Ok(())
            }
            Ordering::Less => {
                self.bft_config.token_symbol[..bytes.len()].copy_from_slice(bytes);
                Ok(())
            }
            Ordering::Greater => Err(BridgeError::SetTokenSymbol(
                "Input string is longer than 16 bytes and needs truncation.".to_string(),
            )),
        }
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
