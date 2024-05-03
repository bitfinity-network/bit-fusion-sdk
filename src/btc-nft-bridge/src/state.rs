use std::path::{Path, PathBuf};

use bitcoin::Network;
use bitcoincore_rpc::{Auth, Client};
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
use crate::interface::store::{BurnRequestStore, MintOrdersStore, NftStore};
use crate::memory::{MEMORY_MANAGER, SIGNER_MEMORY_ID};

type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

pub struct State {
    config: BtcNftBridgeConfig,
    bft_config: NftBridgeConfig,
    signer: SignerStorage,
    mint_orders: MintOrdersStore,
    burn_requests: BurnRequestStore,
    inscriptions: NftStore,
    evm_params: Option<EvmParams>,
}

#[derive(Debug, CandidType, Deserialize)]
pub struct BtcNftBridgeConfig {
    pub network: BitcoinNetwork,
    pub evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
    pub admin: Principal,
    pub ord_url: String,
    pub rpc_config: RpcConfig,
    pub logger: LogSettings,
}

impl Default for BtcNftBridgeConfig {
    fn default() -> Self {
        Self {
            network: BitcoinNetwork::Regtest,
            rpc_config: RpcConfig::default(),
            evm_link: EvmLink::default(),
            signing_strategy: SigningStrategy::Local {
                private_key: [0; 32],
            },
            admin: Principal::management_canister(),
            ord_url: String::new(),
            logger: LogSettings::default(),
        }
    }
}

impl BtcNftBridgeConfig {
    fn validate_indexer_url(&self) -> Result<(), String> {
        if !self.ord_url.starts_with("https") {
            return Err(format!(
                "Indexer URL must be HTTPS. Given: {}",
                self.ord_url
            ));
        }

        Ok(())
    }
}

#[derive(Default, Debug, CandidType, Deserialize)]
pub struct NftBridgeConfig {
    pub erc721_chain_id: u32,
    pub bridge_address: H160,
    pub token_address: H160,
    pub token_name: [u8; 32],
    pub token_symbol: [u8; 16],
}

#[derive(Debug, CandidType, Clone, Deserialize, PartialEq)]
pub struct RpcConfig {
    pub bitcoin_rpc_url: Option<String>,
    pub bitcoin_rpc_username: Option<String>,
    pub bitcoin_rpc_password: Option<String>,
    pub bitcoin_data_dir: Option<PathBuf>,
    pub cookie_file: Option<PathBuf>,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            bitcoin_rpc_url: Some("http://127.0.0.1:18443".to_string()),
            bitcoin_rpc_username: Some("user".to_string()),
            bitcoin_rpc_password: Some("pass".to_string()),
            bitcoin_data_dir: None,
            cookie_file: None,
        }
    }
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
            inscriptions: NftStore::default(),
            evm_params: None,
        }
    }
}

impl State {
    pub fn configure(&mut self, config: BtcNftBridgeConfig) {
        #[cfg(target_family = "wasm")]
        ic_crypto_getrandom_for_wasm::register_custom_getrandom();

        if let Err(err) = config.validate_indexer_url() {
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

    pub fn configure_bft(&mut self, bft_config: NftBridgeConfig) {
        self.bft_config = bft_config;
    }

    pub fn has_nft(&self, reveal_txid: &str) -> bool {
        self.inscriptions.has_inscription(reveal_txid)
    }

    pub fn ord_url(&self) -> String {
        self.config
            .ord_url
            .strip_suffix('/')
            .unwrap_or_else(|| &self.config.ord_url)
            .to_string()
    }

    pub fn erc721_chain_id(&self) -> u32 {
        self.bft_config.erc721_chain_id
    }

    pub fn btc_chain_id(&self) -> u32 {
        match self.config.network {
            BitcoinNetwork::Mainnet => MAINNET_CHAIN_ID,
            BitcoinNetwork::Testnet => TESTNET_CHAIN_ID,
            BitcoinNetwork::Regtest => REGTEST_CHAIN_ID,
        }
    }

    fn bitcoin_rpc_url(&self) -> String {
        self.rpc_config().bitcoin_rpc_url.unwrap_or_default()
    }

    fn bitcoin_credentials(&self) -> anyhow::Result<Auth> {
        if let Some((user, pass)) = &self
            .rpc_config()
            .bitcoin_rpc_username
            .as_ref()
            .zip(self.rpc_config().bitcoin_rpc_password.as_ref())
        {
            Ok(Auth::UserPass((*user).clone(), (*pass).clone()))
        } else {
            Ok(Auth::CookieFile(self.cookie_file()?))
        }
    }

    fn cookie_file(&self) -> anyhow::Result<PathBuf> {
        if let Some(cookie_file) = &self.rpc_config().cookie_file {
            return Ok(cookie_file.clone());
        }

        let path = if let Some(bitcoin_data_dir) = &self.rpc_config().bitcoin_data_dir {
            bitcoin_data_dir.clone()
        } else if cfg!(target_os = "linux") {
            dirs::home_dir()
                .ok_or_else(|| {
                    anyhow::anyhow!("failed to get cookie file path: could not get home dir")
                })?
                .join(".bitcoin")
        } else {
            dirs::data_dir()
                .ok_or_else(|| {
                    anyhow::anyhow!("failed to get cookie file path: could not get data dir")
                })?
                .join("Bitcoin")
        };

        Ok(self.join_btc_network_with_data_dir(path).join(".cookie"))
    }

    fn join_btc_network_with_data_dir(&self, data_dir: impl AsRef<Path>) -> PathBuf {
        match self.btc_network() {
            Network::Testnet => data_dir.as_ref().join("testnet3"),
            Network::Signet => data_dir.as_ref().join("signet"),
            Network::Regtest => data_dir.as_ref().join("regtest"),
            _ => data_dir.as_ref().to_owned(),
        }
    }

    pub fn btc_network(&self) -> Network {
        match self.config.network {
            BitcoinNetwork::Mainnet => Network::Bitcoin,
            BitcoinNetwork::Testnet => Network::Testnet,
            BitcoinNetwork::Regtest => Network::Regtest,
        }
    }

    pub(crate) fn rpc_config(&self) -> RpcConfig {
        self.config.rpc_config.clone()
    }

    pub(crate) fn bitcoin_rpc_client(&self) -> anyhow::Result<Client> {
        let rpc_url = self.bitcoin_rpc_url();
        let bitcoin_credentials = self.bitcoin_credentials()?;

        log::info!("Connecting to Bitcoin Core at {}", self.bitcoin_rpc_url());

        if let Auth::CookieFile(cookie_file) = &bitcoin_credentials {
            log::info!(
                "Using credentials from cookie file at `{}`",
                cookie_file.display()
            );

            anyhow::ensure!(
                cookie_file.is_file(),
                "cookie file `{}` does not exist",
                cookie_file.display()
            );
        }

        Ok(Client::new(&rpc_url, bitcoin_credentials)
            .unwrap_or_else(|_| panic!("failed to connect to Bitcoin Core RPC at `{rpc_url}`")))
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

    pub fn mint_orders_mut(&mut self) -> &mut MintOrdersStore {
        &mut self.mint_orders
    }

    pub fn burn_requests_mut(&mut self) -> &mut BurnRequestStore {
        &mut self.burn_requests
    }

    pub fn inscriptions_mut(&mut self) -> &mut NftStore {
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

    pub fn token_name(&self) -> [u8; 32] {
        self.bft_config.token_name
    }

    pub fn token_symbol(&self) -> [u8; 16] {
        self.bft_config.token_symbol
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
}
