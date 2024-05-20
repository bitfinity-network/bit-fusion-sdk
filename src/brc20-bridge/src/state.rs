use std::cmp::Ordering;

use bitcoin::bip32::ChainCode;
use bitcoin::{Network, PrivateKey, PublicKey};
use candid::{CandidType, Principal};
use did::H160;
use eth_signer::sign_strategy::{SigningStrategy, TxSigner};
use ic_cdk::api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_log::{init_log, LogSettings};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableCell, VirtualMemory};
use inscriber::ecdsa_api::{IcBtcSigner, MasterKey};
use minter_contract_utils::evm_bridge::{EvmInfo, EvmParams};
use minter_contract_utils::evm_link::EvmLink;
use ord_rs::wallet::LocalSigner;
use ord_rs::Wallet;
use serde::Deserialize;

use crate::constant::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};
use crate::interface::bridge_api::BridgeError;
use crate::interface::store::{Brc20Store, BurnRequestStore, MintOrdersStore};
use crate::memory::{MEMORY_MANAGER, SIGNER_MEMORY_ID};

type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

const DEFAULT_DEPOSIT_FEE: u64 = 100_000;

pub struct State {
    config: Brc20BridgeConfig,
    bft_config: BftBridgeConfig,
    signer: SignerStorage,
    mint_orders: MintOrdersStore,
    burn_requests: BurnRequestStore,
    master_key: Option<MasterKey>,
    inscriptions: Brc20Store,
    evm_params: Option<EvmParams>,
}

#[derive(Debug, CandidType, Deserialize)]
pub struct Brc20BridgeConfig {
    pub network: BitcoinNetwork,
    pub evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
    pub admin: Principal,
    pub deposit_fee: u64,
    pub general_indexer: String,
    pub brc20_indexer: String,
    pub logger: LogSettings,
}

impl Default for Brc20BridgeConfig {
    fn default() -> Self {
        Self {
            network: BitcoinNetwork::Regtest,
            evm_link: EvmLink::default(),
            signing_strategy: SigningStrategy::Local {
                private_key: [0; 32],
            },
            admin: Principal::management_canister(),
            deposit_fee: DEFAULT_DEPOSIT_FEE,
            general_indexer: String::new(),
            brc20_indexer: String::new(),
            logger: LogSettings::default(),
        }
    }
}

impl Brc20BridgeConfig {
    fn validate_general_indexer_url(&self) -> Result<(), String> {
        if !self.general_indexer.starts_with("https") {
            return Err(format!(
                "General indexer URL must be HTTPS. Given: {}",
                self.general_indexer
            ));
        }

        Ok(())
    }

    fn validate_brc20_indexer_url(&self) -> Result<(), String> {
        if !self.brc20_indexer.starts_with("https") {
            return Err(format!(
                "BRC20 indexer URL must be HTTPS. Given: {}",
                self.brc20_indexer
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
            master_key: None,
            inscriptions: Brc20Store::default(),
            evm_params: None,
        }
    }
}

impl State {
    pub fn configure(&mut self, config: Brc20BridgeConfig) {
        #[cfg(target_family = "wasm")]
        ic_crypto_getrandom_for_wasm::register_custom_getrandom();

        if let Err(err) = config.validate_general_indexer_url() {
            panic!("Invalid configuration: {err}");
        }

        if let Err(err) = config.validate_brc20_indexer_url() {
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

    pub fn ecdsa_key_id(&self) -> EcdsaKeyId {
        let key_name = match &self.config.signing_strategy {
            SigningStrategy::Local { .. } => "none".to_string(),
            SigningStrategy::ManagementCanister { key_id } => key_id.to_string(),
        };

        EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: key_name,
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.master_key
            .as_ref()
            .expect("master key is not initialized")
            .public_key
    }

    pub fn chain_code(&self) -> ChainCode {
        self.master_key
            .as_ref()
            .expect("master key is not initialized")
            .chain_code
    }

    pub fn wallet(&self) -> Wallet {
        match &self.config.signing_strategy {
            SigningStrategy::Local { private_key } => Wallet::new_with_signer(LocalSigner::new(
                PrivateKey::from_slice(private_key, self.btc_network())
                    .expect("invalid private key"),
            )),
            SigningStrategy::ManagementCanister { .. } => {
                Wallet::new_with_signer(IcBtcSigner::new(self.master_key(), self.btc_network()))
            }
        }
    }

    pub(crate) fn master_key(&self) -> MasterKey {
        self.master_key.clone().expect("ecdsa is not initialized")
    }

    pub fn configure_bft(&mut self, bft_config: BftBridgeConfig) {
        self.bft_config = bft_config;
    }

    pub fn has_brc20(&self, iid: &str) -> bool {
        self.inscriptions.has_inscription(iid)
    }

    pub fn general_indexer_url(&self) -> String {
        self.config
            .general_indexer
            .strip_suffix('/')
            .unwrap_or_else(|| &self.config.general_indexer)
            .to_string()
    }

    pub fn brc20_indexer_url(&self) -> String {
        self.config
            .brc20_indexer
            .strip_suffix('/')
            .unwrap_or_else(|| &self.config.brc20_indexer)
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

    pub fn deposit_fee(&self) -> u64 {
        self.config.deposit_fee
    }
}
