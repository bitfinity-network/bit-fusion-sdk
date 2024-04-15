use bitcoin::bip32::ChainCode;
use bitcoin::{Network, PublicKey};
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

use crate::burn_request_store::BurnRequestStore;
use crate::ledger::Ledger;
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
    burn_request_store: BurnRequestStore,
    evm_params: Option<EvmParams>,
    master_key: Option<MasterKey>,
    ledger: Ledger,
}

struct MasterKey {
    public_key: PublicKey,
    chain_code: ChainCode,
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
            burn_request_store: Default::default(),
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
    pub rune_name: String,
    pub indexer_url: String,
    pub deposit_fee: u64,
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
            rune_name: String::new(),
            indexer_url: String::new(),
            deposit_fee: DEFAULT_DEPOSIT_FEE,
        }
    }
}

impl RuneBridgeConfig {
    fn validate(&self) -> Result<(), String> {
        if self.rune_name.is_empty() {
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
            .clone()
    }

    pub fn chain_code(&self) -> ChainCode {
        self.master_key
            .as_ref()
            .expect("master key is not initialized")
            .chain_code
            .clone()
    }

    pub fn ic_btc_network(&self) -> BitcoinNetwork {
        self.config.network
    }

    pub fn network(&self) -> Network {
        match self.config.network {
            BitcoinNetwork::Mainnet => Network::Bitcoin,
            BitcoinNetwork::Testnet => Network::Testnet,
            BitcoinNetwork::Regtest => Network::Regtest,
        }
    }

    pub fn min_confirmations(&self) -> u32 {
        self.config.min_confirmations
    }

    pub fn rune_name(&self) -> String {
        self.config.rune_name.clone()
    }

    pub fn deposit_fee(&self) -> u64 {
        self.config.deposit_fee
    }

    pub fn indexer_url(&self) -> String {
        self.config
            .indexer_url
            .strip_suffix("/")
            .unwrap_or_else(|| &self.config.indexer_url)
            .to_string()
    }

    pub fn ledger_mut(&mut self) -> &mut Ledger {
        &mut self.ledger
    }

    pub fn signer(&self) -> &SignerStorage {
        &self.signer
    }

    pub fn get_evm_info(&self) -> EvmInfo {
        EvmInfo {
            link: self.config.evm_link.clone(),
            bridge_contract: self.bft_config.bridge_address.clone(),
            params: self.evm_params.clone(),
        }
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

    pub fn get_evm_params(&self) -> &Option<EvmParams> {
        &self.evm_params
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

    pub async fn configure(&mut self, config: RuneBridgeConfig) {
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

    pub fn configure_ecdsa(&mut self, master_key: EcdsaPublicKeyResponse) {
        let chain_code: &[u8] = &master_key.chain_code;
        self.master_key = Some(MasterKey {
            public_key: PublicKey::from_slice(&master_key.public_key)
                .expect("invalid public key slice"),
            chain_code: ChainCode::try_from(chain_code).expect("invalid chain code slice"),
        });
    }

    pub fn configure_bft(&mut self, bft_config: BftBridgeConfig) {
        self.bft_config = bft_config;
    }
    pub fn mint_orders(&self) -> &MintOrdersStore {
        &self.orders_store
    }

    pub fn mint_orders_mut(&mut self) -> &mut MintOrdersStore {
        &mut self.orders_store
    }

    pub fn burn_request_store(&self) -> &BurnRequestStore {
        &self.burn_request_store
    }

    pub fn burn_request_store_mut(&mut self) -> &mut BurnRequestStore {
        &mut self.burn_request_store
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
