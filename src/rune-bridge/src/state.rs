use core::panic;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bitcoin::bip32::ChainCode;
use bitcoin::{FeeRate, Network, PrivateKey, PublicKey};
use bridge_did::init::{RuneBridgeConfig, MIN_INDEXERS};
use bridge_did::runes::{RuneInfo, RuneName};
use eth_signer::sign_strategy::SigningStrategy;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{
    EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyResponse,
};
use ic_exports::ic_kit::ic;
use ord_rs::wallet::LocalSigner;
use ord_rs::Wallet;
use ordinals::RuneId;

use crate::key::{BtcSignerType, IcBtcSigner};
use crate::ledger::UtxoLedger;
use crate::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};

#[derive(Default)]
pub struct RuneState {
    pub(crate) config: RuneBridgeConfig,
    pub(crate) master_key: Option<MasterKey>,
    pub(crate) ledger: UtxoLedger,
    pub(crate) runes: HashMap<RuneName, RuneInfo>,
    pub(crate) fee_rate_state: FeeRateState,
}

#[derive(Debug, Clone)]
pub struct MasterKey {
    pub public_key: PublicKey,
    pub chain_code: ChainCode,
    pub key_id: EcdsaKeyId,
}

pub struct FeeRateState {
    fee_rate: FeeRate,
    /// Last update timestamp in nanoseconds
    last_update_timestamp: u64,
}

impl Default for FeeRateState {
    fn default() -> Self {
        Self {
            fee_rate: FeeRate::ZERO,
            last_update_timestamp: 0,
        }
    }
}

impl RuneState {
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

    /// Chain id to be used for the rune.
    pub fn btc_chain_id(&self) -> u32 {
        match self.config.network {
            BitcoinNetwork::Mainnet => MAINNET_CHAIN_ID,
            BitcoinNetwork::Testnet => TESTNET_CHAIN_ID,
            BitcoinNetwork::Regtest => REGTEST_CHAIN_ID,
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

    /// Returns the number of indexers required to reach consensus.
    pub fn indexer_consensus_threshold(&self) -> u8 {
        self.config.indexer_consensus_threshold
    }

    /// Sets the number of indexers required to reach consensus.
    pub fn set_indexer_consensus_threshold(&mut self, threshold: u8) {
        self.config.indexer_consensus_threshold = threshold;
    }

    pub fn mempool_timeout(&self) -> Duration {
        self.config.mempool_timeout
    }

    /// Update fee rate and the last update timestamp.
    pub fn update_fee_rate(&mut self, fee_rate: FeeRate) {
        self.fee_rate_state.fee_rate = fee_rate;
        self.fee_rate_state.last_update_timestamp = ic::time();
    }

    /// Fee rate used by the canister.
    pub fn fee_rate(&self) -> FeeRate {
        self.fee_rate_state.fee_rate
    }

    /// Elapsed time since the last fee rate update. (nano seconds)
    pub fn last_fee_rate_update_elapsed(&self) -> Duration {
        ic::time()
            .checked_sub(self.fee_rate_state.last_update_timestamp)
            .map(Duration::from_nanos)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::ic_kit::MockContext;

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
            indexer_consensus_threshold: 1,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_configure_with_trailing_slash() {
        MockContext::new().inject();
        let config = RuneBridgeConfig {
            indexer_urls: HashSet::from_iter(vec!["https://url.com/".to_string()]),
            no_of_indexers: 1,
            signing_strategy: SigningStrategy::Local {
                private_key: [1; 32],
            },
            indexer_consensus_threshold: 1,
            ..Default::default()
        };
        let mut state = RuneState::default();
        state.configure(config);

        assert_eq!(
            state.indexer_urls(),
            HashSet::from_iter(vec![String::from("https://url.com")])
        );
    }

    #[test]
    fn test_configure_indexers_valid() {
        let mut state = RuneState::default();
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
        let mut state = RuneState::default();
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
        let mut state = RuneState::default();
        let indexer_urls: HashSet<String> = HashSet::new();

        state.configure_indexers(MIN_INDEXERS - 1, indexer_urls);
    }

    #[test]
    #[should_panic(expected = "number of indexers must be at least")]
    fn test_configure_indexers_fewer_than_urls() {
        let mut state = RuneState::default();
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
        let mut state = RuneState::default();
        let urls = vec!["http://indexer1.com", "http://indexer2.com"];
        let indexer_urls: HashSet<String> = urls.into_iter().map(String::from).collect();

        state.configure_indexers(3, indexer_urls.clone());

        assert_eq!(state.config.no_of_indexers, 3);
        assert_eq!(state.config.indexer_urls, indexer_urls);
    }

    #[test]
    fn test_should_update_and_read_fee_rate() {
        let ctx = MockContext::new().inject();
        let mut state = RuneState::default();

        assert_eq!(state.fee_rate(), FeeRate::ZERO);
        assert_eq!(state.fee_rate_state.last_update_timestamp, 0);

        let fee_rate = FeeRate::from_sat_per_vb(1000).unwrap();
        state.update_fee_rate(fee_rate);

        assert_eq!(state.fee_rate(), fee_rate);
        ctx.add_time(Duration::from_secs(1).as_nanos() as u64);
        assert!(state.last_fee_rate_update_elapsed() >= Duration::from_secs(1));
    }
}
