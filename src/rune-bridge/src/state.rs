use bitcoin::bip32::ChainCode;
use bitcoin::{Network, PrivateKey, PublicKey};
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
use minter_contract_utils::evm_bridge::{EvmInfo, EvmParams};
use minter_contract_utils::evm_link::EvmLink;
use ord_rs::{Wallet, WalletType};
use ordinals::RuneId;

use crate::key::IcBtcSigner;
use crate::ledger::UtxoLedger;
use crate::memory::{MEMORY_MANAGER, SIGNER_MEMORY_ID};
use crate::orders_store::MintOrdersStore;
use crate::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};

type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

const DEFAULT_DEPOSIT_FEE: u64 = 100_000;

pub struct State {
    config: RuneBridgeConfig,
    bft_config: BftBridgeConfig,
    signer: SignerStorage,
    orders_store: MintOrdersStore,
    evm_params: Option<EvmParams>,
    master_key: Option<MasterKey>,
    ledger: UtxoLedger,
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
            orders_store: Default::default(),
            evm_params: None,
            master_key: None,
            ledger: Default::default(),
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
    pub rune_info: RuneInfo,
    pub indexer_url: String,
    pub deposit_fee: u64,
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct RuneInfo {
    pub name: String,
    pub block: u64,
    pub tx: u32,
}

impl RuneInfo {
    pub fn id(&self) -> RuneId {
        RuneId {
            block: self.block,
            tx: self.tx,
        }
    }
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
            rune_info: RuneInfo {
                name: "".to_string(),
                block: 0,
                tx: 0,
            },
            indexer_url: String::new(),
            deposit_fee: DEFAULT_DEPOSIT_FEE,
        }
    }
}

impl RuneBridgeConfig {
    fn validate(&self) -> Result<(), String> {
        if self.rune_info.name.is_empty() {
            return Err("Rune name is empty".to_string());
        }

        if self.indexer_url.is_empty() {
            return Err("Indexer url is empty".to_string());
        }

        if !self.indexer_url.starts_with("https") {
            return Err(format!(
                "Indexer url must specify https url, but give value is: {}",
                self.indexer_url
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

    /// Name of the rune the canister works with.
    pub fn rune_name(&self) -> String {
        self.config.rune_info.name.clone()
    }

    /// Id of the rune the canister works with.
    pub fn rune_id(&self) -> RuneId {
        self.config.rune_info.id()
    }

    fn master_key(&self) -> MasterKey {
        self.master_key.clone().expect("ecdsa is not initialized")
    }

    /// Derived public key with the given derivation path.
    pub fn der_public_key(&self, derivation_path: &[Vec<u8>]) -> PublicKey {
        IcBtcSigner::new(self.master_key(), self.network(), derivation_path.to_vec()).public_key()
    }

    /// Returns `WalletType` structure to be used to sign transactions with the given
    /// derivation path.
    pub fn wallet_type(&self, derivation_path: Vec<Vec<u8>>) -> WalletType {
        match &self.config.signing_strategy {
            SigningStrategy::Local { private_key } => WalletType::Local {
                private_key: PrivateKey::from_slice(private_key, self.network())
                    .expect("Invalid PK"),
            },
            SigningStrategy::ManagementCanister { .. } => WalletType::External {
                signer: Box::new(IcBtcSigner::new(
                    self.master_key(),
                    self.network(),
                    derivation_path,
                )),
            },
        }
    }

    /// Wallet to be used to sign transactions with the given derivation path.
    pub fn wallet(&self, derivation_path: Vec<Vec<u8>>) -> Wallet {
        Wallet::new_with_signer(self.wallet_type(derivation_path))
    }

    /// BTC fee in SATs for a deposit request.
    pub fn deposit_fee(&self) -> u64 {
        self.config.deposit_fee
    }

    /// Url of the `ord` indexer this canister rely on.
    pub fn indexer_url(&self) -> String {
        self.config
            .indexer_url
            .strip_suffix('/')
            .unwrap_or_else(|| &self.config.indexer_url)
            .to_string()
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
    pub fn configure(&mut self, config: RuneBridgeConfig) {
        if let Err(err) = config.validate() {
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

    /// Configures the link to BFT bridge contract.
    pub fn configure_bft(&mut self, bft_config: BftBridgeConfig) {
        self.bft_config = bft_config;
    }

    /// Store of the signed mint orders.
    pub fn mint_orders(&self) -> &MintOrdersStore {
        &self.orders_store
    }

    /// Mutable reference to the signed mint orders store.
    pub fn mint_orders_mut(&mut self) -> &mut MintOrdersStore {
        &mut self.orders_store
    }

    /// Name of the wrapped token.
    pub fn token_name(&self) -> [u8; 32] {
        self.bft_config.token_name
    }

    /// Symbol of the wrapped token.
    pub fn token_symbol(&self) -> [u8; 16] {
        self.bft_config.token_symbol
    }

    /// Decimals of the wrapped token.
    pub fn decimals(&self) -> u8 {
        self.bft_config.decimals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexer_url_stripping() {
        let config = RuneBridgeConfig {
            indexer_url: "https://url.com".to_string(),
            ..Default::default()
        };
        let state = State {
            config,
            ..Default::default()
        };

        assert_eq!(state.indexer_url(), "https://url.com".to_string());

        let config = RuneBridgeConfig {
            indexer_url: "https://url.com/".to_string(),
            ..Default::default()
        };
        let state = State {
            config,
            ..Default::default()
        };

        assert_eq!(state.indexer_url(), "https://url.com".to_string());
    }
}
