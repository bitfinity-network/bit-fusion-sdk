use core::panic;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bitcoin::bip32::ChainCode;
use bitcoin::{Network, PrivateKey, PublicKey};
use bridge_utils::evm_bridge::{EvmInfo, EvmParams};
use bridge_utils::evm_link::EvmLink;
use candid::{CandidType, Deserialize, Principal};
use did::H160;
use eth_signer::sign_strategy::{SigningStrategy, TxSigner};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{
    EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyResponse,
};
use ic_log::{init_log, LogSettings};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableCell, VirtualMemory};
use ord_rs::wallet::LocalSigner;
use ord_rs::Wallet;
use ordinals::RuneId;

use crate::key::{BtcSignerType, IcBtcSigner};
use crate::ledger::UtxoLedger;
use crate::memory::{MEMORY_MANAGER, SIGNER_MEMORY_ID};
use crate::rune_info::{RuneInfo, RuneName};
use crate::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};

type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

const DEFAULT_DEPOSIT_FEE: u64 = 100_000;
const DEFAULT_MEMPOOL_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

/// Minimum number of indexers required to start the bridge.
const MIN_INDEXERS: u8 = 2;

pub struct State {
    pub(crate) config: RuneBridgeConfig,
    pub(crate) bft_config: BftBridgeConfig,
    pub(crate) signer: SignerStorage,
    pub(crate) evm_params: Option<EvmParams>,
    pub(crate) master_key: Option<MasterKey>,
    pub(crate) ledger: UtxoLedger,
    pub(crate) runes: HashMap<RuneName, RuneInfo>,
}

#[derive(Debug, Clone)]
pub struct MasterKey {
    pub public_key: PublicKey,
    pub chain_code: ChainCode,
    pub key_id: EcdsaKeyId,
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
            evm_params: None,
            master_key: None,
            ledger: Default::default(),
            runes: Default::default(),
        }
    }
}

#[derive(Debug, CandidType, Deserialize)]
pub struct RuneBridgeConfig {
    pub network: BitcoinNetwork,
    pub evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
    pub admin: Principal,
    pub log_settings: LogSettings,
    pub min_confirmations: u32,
    pub no_of_indexers: u8,
    pub indexer_urls: HashSet<String>,
    pub deposit_fee: u64,
    pub mempool_timeout: Duration,
}

impl Default for RuneBridgeConfig {
    fn default() -> Self {
        Self {
            network: BitcoinNetwork::Regtest,
            evm_link: EvmLink::default(),
            signing_strategy: SigningStrategy::Local {
                private_key: [0; 32],
            },
            admin: Principal::management_canister(),
            log_settings: LogSettings::default(),
            min_confirmations: 12,
            no_of_indexers: MIN_INDEXERS,
            indexer_urls: HashSet::default(),
            deposit_fee: DEFAULT_DEPOSIT_FEE,
            mempool_timeout: DEFAULT_MEMPOOL_TIMEOUT,
        }
    }
}

impl RuneBridgeConfig {
    fn validate(&self) -> Result<(), String> {
        if self.indexer_urls.is_empty() {
            return Err("Indexer url is empty".to_string());
        }

        if self.indexer_urls.len() != self.no_of_indexers as usize {
            return Err(format!(
                "Number of indexers ({}) required does not match number of indexer urls ({})",
                self.no_of_indexers,
                self.indexer_urls.len()
            ));
        }

        if self
            .indexer_urls
            .iter()
            .any(|url| !url.starts_with("https"))
        {
            return Err("Indexer url must specify https url".to_string());
        }

        Ok(())
    }
}

#[derive(Default, Debug, CandidType, Deserialize)]
pub struct BftBridgeConfig {
    pub erc20_chain_id: u32,
    pub bridge_address: H160,
}

impl State {
    /// Returns id of the IC ECDSA key used by the canister.
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

    pub fn runes(&self) -> &HashMap<RuneName, RuneInfo> {
        &self.runes
    }

    pub fn rune_info(&self, rune_id: RuneId) -> Option<RuneInfo> {
        self.runes
            .values()
            .find(|rune_info| rune_info.id() == rune_id)
            .copied()
    }

    pub fn update_rune_list(&mut self, runes: HashMap<RuneName, RuneInfo>) {
        self.runes = runes;
    }

    /// Returns master public key of the canister.
    pub fn public_key(&self) -> PublicKey {
        self.master_key
            .as_ref()
            .expect("master key is not initialized")
            .public_key
    }

    /// Returns master chain code of the canister. Used for public key derivation.
    pub fn chain_code(&self) -> ChainCode {
        self.master_key
            .as_ref()
            .expect("master key is not initialized")
            .chain_code
    }

    /// Returns BTC network the canister works with (IC style).
    pub fn ic_btc_network(&self) -> BitcoinNetwork {
        self.config.network
    }

    /// Returns BTC network the canister works with (BTC style).
    pub fn network(&self) -> Network {
        match self.config.network {
            BitcoinNetwork::Mainnet => Network::Bitcoin,
            BitcoinNetwork::Testnet => Network::Testnet,
            BitcoinNetwork::Regtest => Network::Regtest,
        }
    }

    /// Minimum number of confirmations the canister requires to consider a transaction to be confirmed.
    pub fn min_confirmations(&self) -> u32 {
        self.config.min_confirmations
    }

    fn master_key(&self) -> MasterKey {
        self.master_key.clone().expect("ecdsa is not initialized")
    }

    pub fn btc_signer(&self) -> BtcSignerType {
        match &self.config.signing_strategy {
            SigningStrategy::Local { private_key } => BtcSignerType::Local(LocalSigner::new(
                PrivateKey::from_slice(private_key, self.network()).expect("invalid private key"),
            )),
            SigningStrategy::ManagementCanister { .. } => {
                BtcSignerType::Ic(IcBtcSigner::new(self.master_key(), self.network()))
            }
        }
    }

    /// Wallet to be used to sign transactions with the given derivation path.
    pub fn wallet(&self) -> Wallet {
        Wallet::new_with_signer(self.btc_signer())
    }

    /// BTC fee in SATs for a deposit request.
    pub fn deposit_fee(&self) -> u64 {
        self.config.deposit_fee
    }

    /// Url of the `ord` indexer this canister rely on.
    pub fn indexer_urls(&self) -> HashSet<String> {
        self.config.indexer_urls.clone()
    }

    /// Utxo ledger.
    pub fn ledger(&self) -> &UtxoLedger {
        &self.ledger
    }

    /// Mutable reference to the utxo ledger.
    pub fn ledger_mut(&mut self) -> &mut UtxoLedger {
        &mut self.ledger
    }

    /// Eth transaction signer.
    pub fn signer(&self) -> &SignerStorage {
        &self.signer
    }

    /// Current EVM link state.
    pub fn get_evm_info(&self) -> EvmInfo {
        EvmInfo {
            link: self.config.evm_link.clone(),
            bridge_contract: self.bft_config.bridge_address.clone(),
            params: self.evm_params.clone(),
        }
    }

    /// Chain id of the EVM.
    pub fn erc20_chain_id(&self) -> u32 {
        self.bft_config.erc20_chain_id
    }

    /// Chain id to be used for the rune.
    pub fn btc_chain_id(&self) -> u32 {
        match self.config.network {
            BitcoinNetwork::Mainnet => MAINNET_CHAIN_ID,
            BitcoinNetwork::Testnet => TESTNET_CHAIN_ID,
            BitcoinNetwork::Regtest => REGTEST_CHAIN_ID,
        }
    }

    /// Returns EVM parameters.
    pub fn get_evm_params(&self) -> &Option<EvmParams> {
        &self.evm_params
    }

    /// Updates EVM parameters with the given closure.
    pub fn update_evm_params(&mut self, f: impl FnOnce(&mut Option<EvmParams>)) {
        f(&mut self.evm_params)
    }

    /// Admin principal of the canister.
    pub fn admin(&self) -> Principal {
        self.config.admin
    }

    /// Panics if the current caller is not admin of the canister.
    pub fn check_admin(&self, caller: Principal) {
        if caller != self.admin() {
            panic!("access denied");
        }
    }

    /// Validates the given configuration and sets it to the state. Panics in case the configuration
    /// is invalid.
    pub fn configure(&mut self, mut config: RuneBridgeConfig) {
        if let Err(err) = config.validate() {
            panic!("Invalid configuration: {err}");
        }

        config.indexer_urls = config
            .indexer_urls
            .iter()
            .map(|url| url.strip_suffix('/').unwrap_or(url).to_owned())
            .collect();

        let signer = config
            .signing_strategy
            .clone()
            .make_signer(0)
            .expect("Failed to create signer");
        let stable = SignerStorage::new(MEMORY_MANAGER.with(|mm| mm.get(SIGNER_MEMORY_ID)), signer)
            .expect("failed to init signer in stable memory");
        self.signer = stable;

        init_log(&config.log_settings).expect("failed to init logger");

        self.config = config;
    }

    /// Updates the ecdsa signing configuration with the given master key information.
    ///
    /// This configuration is used to derive public keys for different user addresses, so this
    /// configuration must be set before any of the transactions can be processed.
    pub fn configure_ecdsa(&mut self, master_key: EcdsaPublicKeyResponse) {
        let chain_code: &[u8] = &master_key.chain_code;
        self.master_key = Some(MasterKey {
            public_key: PublicKey::from_slice(&master_key.public_key)
                .expect("invalid public key slice"),
            chain_code: ChainCode::try_from(chain_code).expect("invalid chain code slice"),
            key_id: self.ecdsa_key_id(),
        });
    }

    pub fn configure_indexers(&mut self, no_of_indexers: u8, indexer_urls: HashSet<String>) {
        if no_of_indexers < MIN_INDEXERS {
            panic!("number of indexers must be at least {}", MIN_INDEXERS)
        }

        if no_of_indexers < indexer_urls.len() as u8 {
            panic!(
                "number of indexers must be at least {}",
                indexer_urls.len() as u8,
            );
        }

        self.config.indexer_urls = indexer_urls
            .iter()
            .map(|url| url.strip_suffix('/').unwrap_or(url).to_owned())
            .collect();

        self.config.no_of_indexers = no_of_indexers;
    }

    /// Configures the link to BFT bridge contract.
    pub fn configure_bft(&mut self, bft_config: BftBridgeConfig) {
        self.bft_config = bft_config;
    }

    pub fn mempool_timeout(&self) -> Duration {
        self.config.mempool_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_indexer_urls() {
        let config = RuneBridgeConfig {
            indexer_urls: HashSet::new(),
            no_of_indexers: 1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_mismatched_number_of_indexers() {
        let config = RuneBridgeConfig {
            indexer_urls: HashSet::from_iter(vec!["https://url.com".to_string()]),
            no_of_indexers: 2,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_non_https_url() {
        let config = RuneBridgeConfig {
            indexer_urls: HashSet::from_iter(vec!["http://url.com".to_string()]),
            no_of_indexers: 1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_success() {
        let config = RuneBridgeConfig {
            indexer_urls: HashSet::from_iter(vec!["https://url.com".to_string()]),
            no_of_indexers: 1,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_configure_with_trailing_slash() {
        let config = RuneBridgeConfig {
            indexer_urls: HashSet::from_iter(vec!["https://url.com/".to_string()]),
            no_of_indexers: 1,
            signing_strategy: SigningStrategy::Local {
                private_key: [1; 32],
            },
            ..Default::default()
        };
        let mut state = State::default();
        state.configure(config);

        assert_eq!(
            state.indexer_urls(),
            HashSet::from_iter(vec![String::from("https://url.com")])
        );
    }

    #[test]
    fn test_configure_indexers_valid() {
        let mut state = State::default();
        let urls = vec![
            "http://indexer1.com",
            "http://indexer2.com",
            "http://indexer3.com",
        ];
        let indexer_urls: HashSet<String> = urls.into_iter().map(String::from).collect();

        state.configure_indexers(3, indexer_urls.clone());

        assert_eq!(state.config.no_of_indexers, 3);
        assert_eq!(state.config.indexer_urls, indexer_urls);
    }

    #[test]
    fn test_configure_indexers_strip_trailing_slash() {
        let mut state = State::default();
        let urls = vec![
            "http://indexer1.com/",
            "http://indexer2.com",
            "http://indexer3.com/",
        ];
        let indexer_urls: HashSet<String> = urls.into_iter().map(String::from).collect();

        state.configure_indexers(3, indexer_urls);

        assert_eq!(
            state.config.indexer_urls,
            HashSet::from([
                "http://indexer1.com".to_string(),
                "http://indexer2.com".to_string(),
                "http://indexer3.com".to_string(),
            ])
        );
    }

    #[test]
    #[should_panic(expected = "number of indexers must be at least")]
    fn test_configure_indexers_too_few_indexers() {
        let mut state = State::default();
        let indexer_urls: HashSet<String> = HashSet::new();

        state.configure_indexers(MIN_INDEXERS - 1, indexer_urls);
    }

    #[test]
    #[should_panic(expected = "number of indexers must be at least")]
    fn test_configure_indexers_fewer_than_urls() {
        let mut state = State::default();
        let urls = vec![
            "http://indexer1.com",
            "http://indexer2.com",
            "http://indexer3.com",
        ];
        let indexer_urls: HashSet<String> = urls.into_iter().map(String::from).collect();

        state.configure_indexers(2, indexer_urls);
    }

    #[test]
    fn test_configure_indexers_more_than_urls() {
        let mut state = State::default();
        let urls = vec!["http://indexer1.com", "http://indexer2.com"];
        let indexer_urls: HashSet<String> = urls.into_iter().map(String::from).collect();

        state.configure_indexers(3, indexer_urls.clone());

        assert_eq!(state.config.no_of_indexers, 3);
        assert_eq!(state.config.indexer_urls, indexer_urls);
    }
}
