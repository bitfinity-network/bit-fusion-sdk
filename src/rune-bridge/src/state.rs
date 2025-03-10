mod config;
mod master_key;

use core::panic;
use std::collections::HashMap;
use std::time::Duration;

use bitcoin::bip32::ChainCode;
use bitcoin::{FeeRate, Network, PrivateKey, PublicKey};
use bridge_canister::memory::MEMORY_MANAGER;
use bridge_did::init::{IndexerType, MIN_INDEXERS, RuneBridgeConfig};
use bridge_did::runes::{RuneInfo, RuneName};
use eth_signer::sign_strategy::SigningStrategy;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{
    EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyResponse,
};
use ic_exports::ic_kit::ic;
use ic_stable_structures::VirtualMemory;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ord_rs::Wallet;
use ord_rs::wallet::LocalSigner;
use ordinals::RuneId;

use self::config::RuneBridgeConfigStorage;
pub use self::master_key::MasterKey;
use self::master_key::MasterKeyStorage;
use crate::key::{BtcSignerType, IcBtcSigner};
use crate::ledger::UtxoLedger;
use crate::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};

pub struct RuneState {
    pub(crate) config: RuneBridgeConfigStorage<VirtualMemory<DefaultMemoryImpl>>,
    pub(crate) master_key: MasterKeyStorage<VirtualMemory<DefaultMemoryImpl>>,
    pub(crate) ledger: UtxoLedger<VirtualMemory<DefaultMemoryImpl>>,
    pub(crate) runes: HashMap<RuneName, RuneInfo>,
    pub(crate) fee_rate_state: FeeRateState,
}

impl Default for RuneState {
    fn default() -> Self {
        MEMORY_MANAGER.with(|memory_manager| Self {
            config: RuneBridgeConfigStorage::new(memory_manager),
            fee_rate_state: FeeRateState::default(),
            ledger: UtxoLedger::new(memory_manager),
            master_key: MasterKeyStorage::new(memory_manager),
            runes: HashMap::default(),
        })
    }
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
    pub fn ecdsa_key_id(&self, signing_strategy: &SigningStrategy) -> EcdsaKeyId {
        let key_name = match signing_strategy {
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
    pub fn public_key(&self) -> Option<PublicKey> {
        self.master_key
            .get()
            .as_ref()
            .and_then(|key| key.public_key().ok())
    }

    /// Returns master chain code of the canister. Used for public key derivation.
    pub fn chain_code(&self) -> Option<ChainCode> {
        self.master_key.get().as_ref().map(|key| key.chain_code())
    }

    /// Returns BTC network the canister works with (IC style).
    pub fn ic_btc_network(&self) -> BitcoinNetwork {
        self.config.get().network
    }

    pub fn utxo_cache_timeout(&self) -> Duration {
        Duration::from_secs(self.config.get().btc_cache_timeout_secs.unwrap_or(0) as u64)
    }

    /// Returns BTC network the canister works with (BTC style).
    pub fn network(&self) -> Network {
        match self.config.get().network {
            BitcoinNetwork::Mainnet => Network::Bitcoin,
            BitcoinNetwork::Testnet => Network::Testnet,
            BitcoinNetwork::Regtest => Network::Regtest,
        }
    }

    /// Minimum number of confirmations the canister requires to consider a transaction to be confirmed.
    pub fn min_confirmations(&self) -> u32 {
        self.config.get().min_confirmations
    }

    /// Master key of the canister.
    fn master_key(&self) -> Option<MasterKey> {
        self.master_key.get().clone()
    }

    pub fn btc_signer(&self, signing_strategy: &SigningStrategy) -> Option<BtcSignerType> {
        Some(match signing_strategy {
            SigningStrategy::Local { private_key } => BtcSignerType::Local(LocalSigner::new(
                PrivateKey::from_slice(private_key, self.network()).expect("invalid private key"),
            )),
            SigningStrategy::ManagementCanister { .. } => {
                BtcSignerType::Ic(IcBtcSigner::new(self.master_key()?, self.network()))
            }
        })
    }

    /// Wallet to be used to sign transactions with the given derivation path.
    pub fn wallet(&self, signing_strategy: &SigningStrategy) -> Option<Wallet> {
        Some(Wallet::new_with_signer(self.btc_signer(signing_strategy)?))
    }

    /// BTC fee in SATs for a deposit request.
    pub fn deposit_fee(&self) -> u64 {
        self.config.get().deposit_fee
    }

    /// Configuration of the indexers
    pub fn indexers_config(&self) -> Vec<IndexerType> {
        self.config.get().indexers.clone()
    }

    /// Utxo ledger.
    pub fn ledger(&self) -> &UtxoLedger<VirtualMemory<DefaultMemoryImpl>> {
        &self.ledger
    }

    /// Mutable reference to the utxo ledger.
    pub fn ledger_mut(&mut self) -> &mut UtxoLedger<VirtualMemory<DefaultMemoryImpl>> {
        &mut self.ledger
    }

    /// Chain id to be used for the rune.
    pub fn btc_chain_id(&self) -> u32 {
        match self.config.get().network {
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

        config
            .indexers
            .iter_mut()
            .for_each(|config| config.normalize());

        self.config.set(config);
    }

    /// Updates the ecdsa signing configuration with the given master key information.
    ///
    /// This configuration is used to derive public keys for different user addresses, so this
    /// configuration must be set before any of the transactions can be processed.
    pub fn configure_ecdsa(
        &mut self,
        master_key: EcdsaPublicKeyResponse,
        key_id: EcdsaKeyId,
    ) -> Result<(), String> {
        if master_key.chain_code.len() != 32 {
            return Err("invalid chain code length".to_string());
        }
        let chain_code: [u8; 32] = master_key
            .chain_code
            .try_into()
            .map_err(|e| format!("invalid chain code: {e:?}"))?;

        let master_key = MasterKey::new(
            PublicKey::from_slice(&master_key.public_key)
                .map_err(|e| format!("invalid public key slice: {e}"))?,
            ChainCode::from(chain_code),
            key_id,
        );

        self.master_key.set(master_key);

        Ok(())
    }

    pub fn configure_indexers(&mut self, mut indexers: Vec<IndexerType>) {
        if indexers.len() < MIN_INDEXERS {
            panic!("number of indexers must be at least {}", MIN_INDEXERS)
        }

        indexers.iter_mut().for_each(|config| config.normalize());
        self.config
            .with_borrow_mut(move |config| config.indexers = indexers);
    }

    pub fn mempool_timeout(&self) -> Duration {
        self.config.get().mempool_timeout
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

    /// Returns the number of indexers required to reach consensus.
    pub fn indexer_consensus_threshold(&self) -> u8 {
        self.config.get().indexer_consensus_threshold
    }

    /// Sets the number of indexers required to reach consensus.
    pub fn set_indexer_consensus_threshold(&mut self, threshold: u8) {
        self.config
            .with_borrow_mut(|config| config.indexer_consensus_threshold = threshold);
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::ic_kit::MockContext;

    use super::*;

    #[test]
    fn test_validate_empty_indexers() {
        let config = RuneBridgeConfig {
            indexers: vec![],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_non_https_url() {
        let config = RuneBridgeConfig {
            indexers: vec![IndexerType::OrdHttp {
                url: "http://url.com".to_string(),
            }],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_success() {
        let config = RuneBridgeConfig {
            indexers: vec![IndexerType::OrdHttp {
                url: "https://url.com".to_string(),
            }],
            indexer_consensus_threshold: 1,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_configure_with_trailing_slash() {
        MockContext::new().inject();
        let config = RuneBridgeConfig {
            indexers: vec![IndexerType::OrdHttp {
                url: "https://url.com/".to_string(),
            }],
            indexer_consensus_threshold: 1,
            ..Default::default()
        };
        let mut state = RuneState::default();
        state.configure(config);

        assert_eq!(
            state.indexers_config(),
            vec![IndexerType::OrdHttp {
                url: "https://url.com".to_string()
            }],
        );
    }

    #[test]
    fn test_configure_indexers_valid() {
        let mut state = RuneState::default();
        let indexers = vec![
            IndexerType::OrdHttp {
                url: "https://indexer1.com".to_string(),
            },
            IndexerType::OrdHttp {
                url: "https://indexer2.com".to_string(),
            },
            IndexerType::OrdHttp {
                url: "https://indexer3.com".to_string(),
            },
        ];

        state.configure_indexers(indexers.clone());

        assert_eq!(state.config.get().indexers, indexers);
    }

    #[test]
    fn test_configure_indexers_strip_trailing_slash() {
        let mut state = RuneState::default();
        let indexers = vec![
            IndexerType::OrdHttp {
                url: "https://indexer1.com/".to_string(),
            },
            IndexerType::OrdHttp {
                url: "https://indexer2.com/".to_string(),
            },
            IndexerType::OrdHttp {
                url: "https://indexer3.com".to_string(),
            },
        ];

        state.configure_indexers(indexers);

        assert_eq!(
            state.config.get().indexers,
            vec![
                IndexerType::OrdHttp {
                    url: "https://indexer1.com".to_string(),
                },
                IndexerType::OrdHttp {
                    url: "https://indexer2.com".to_string(),
                },
                IndexerType::OrdHttp {
                    url: "https://indexer3.com".to_string(),
                },
            ]
        );
    }

    #[test]
    #[should_panic(expected = "number of indexers must be at least")]
    fn test_configure_indexers_too_few_indexers() {
        let mut state = RuneState::default();
        let indexers = vec![];

        state.configure_indexers(indexers);
    }

    #[test]
    #[should_panic(expected = "number of indexers must be at least")]
    fn test_configure_indexers_fewer_than_urls() {
        let mut state = RuneState::default();
        let indexers = vec![IndexerType::OrdHttp {
            url: "http://indexer1.com".to_string(),
        }];

        state.configure_indexers(indexers);
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
