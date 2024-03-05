use crate::memory::{MEMORY_MANAGER, SIGNER_MEMORY_ID};
use crate::orders_store::OrdersStore;
use candid::{CandidType, Principal};
use did::H160;
use eth_signer::sign_strategy::{ManagementCanisterSigner, SigningKeyId, TxSigner};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableCell, VirtualMemory};
use minter_contract_utils::evm_bridge::{EvmInfo, EvmParams};
use minter_contract_utils::evm_link::EvmLink;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;
use serde::Deserialize;

const MAINNET_CHAIN_ID: u32 = 0;
const TESTNET_CHAIN_ID: u32 = 1;
const REGTEST_CHAIN_ID: u32 = 2;

type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

pub struct State {
    config: BtcBridgeConfig,
    signer: SignerStorage,
    orders_store: OrdersStore,
    evm_params: Option<EvmParams>,
}

#[derive(Debug, CandidType, Deserialize)]
pub struct BtcBridgeConfig {
    ck_btc_minter: Principal,
    ck_btc_ledger: Principal,
    erc20_token_id: u32,
    network: BitcoinNetwork,
    evm_link: EvmLink,
    bridge_address: H160,
    token_name: [u8; 32],
    token_symbol: [u8; 16],
    decimals: u8,
}

impl Default for BtcBridgeConfig {
    fn default() -> Self {
        Self {
            ck_btc_minter: Principal::anonymous(),
            ck_btc_ledger: Principal::anonymous(),
            erc20_token_id: 0,
            network: BitcoinNetwork::Regtest,
            evm_link: EvmLink::default(),
            bridge_address: H160::default(),
            token_name: [0; 32],
            token_symbol: [0; 16],
            decimals: 0,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        let default_signer =
            TxSigner::ManagementCanister(ManagementCanisterSigner::new(SigningKeyId::Test, vec![]));
        let signer = SignerStorage::new(
            MEMORY_MANAGER.with(|mm| mm.get(SIGNER_MEMORY_ID)),
            default_signer,
        )
        .expect("failed to initialize transaction signer");

        Self {
            config: Default::default(),
            signer,
            orders_store: Default::default(),
            evm_params: None,
        }
    }
}

impl State {
    pub fn configure(&mut self, config: BtcBridgeConfig) {
        self.config = config;
    }

    pub fn ck_btc_minter(&self) -> Principal {
        self.config.ck_btc_minter
    }

    pub fn ck_btc_ledger(&self) -> Principal {
        self.config.ck_btc_ledger
    }

    pub fn erc20_token_id(&self) -> u32 {
        self.config.erc20_token_id
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

    pub fn push_mint_order(&mut self, sender: Id256, nonce: u32, mint_order: SignedMintOrder) {
        self.orders_store.push_mint_order(sender, nonce, mint_order);
    }

    pub fn get_evm_info(&self) -> EvmInfo {
        EvmInfo {
            link: self.config.evm_link.clone(),
            bridge_contract: self.config.bridge_address.clone(),
            params: self.evm_params.clone(),
        }
    }

    pub fn get_evm_params(&self) -> &Option<EvmParams> {
        &self.evm_params
    }

    pub fn token_name(&self) -> [u8; 32] {
        self.config.token_name
    }

    pub fn token_symbol(&self) -> [u8; 16] {
        self.config.token_symbol
    }

    pub fn decimals(&self) -> u8 {
        self.config.decimals
    }
}
