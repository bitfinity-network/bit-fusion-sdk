use bridge_canister::memory::memory_by_id;
use bridge_did::init::btc::{BitcoinConnection, WrappedTokenConfig};
use candid::Principal;
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, VirtualMemory};

use crate::memory::{BTC_CONFIG_MEMORY_ID, WRAPPED_TOKEN_CONFIG_MEMORY_ID};
use crate::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};

pub struct State {
    pub btc_config: StableCell<BitcoinConnection, VirtualMemory<DefaultMemoryImpl>>,
    pub wrapped_token_config: StableCell<WrappedTokenConfig, VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            wrapped_token_config: StableCell::new(
                memory_by_id(WRAPPED_TOKEN_CONFIG_MEMORY_ID),
                WrappedTokenConfig::default(),
            )
            .expect("stable memory config initialization failed"),
            btc_config: StableCell::new(
                memory_by_id(BTC_CONFIG_MEMORY_ID),
                BitcoinConnection::default(),
            )
            .expect("stable memory config initialization failed"),
        }
    }
}

impl State {
    pub fn configure_btc(&mut self, config: BitcoinConnection) {
        self.btc_config.set(config).expect("failed to set config");
    }

    pub fn configure_wrapped_token(&mut self, config: WrappedTokenConfig) {
        self.wrapped_token_config
            .set(config)
            .expect("failed to set wrapped token config");
    }

    pub fn ck_btc_minter(&self) -> Principal {
        self.with_btc_config(|config| config.ckbtc_minter())
    }

    pub fn ck_btc_ledger(&self) -> Principal {
        self.with_btc_config(|config| config.ckbtc_ledger())
    }

    pub fn btc_chain_id(&self) -> u32 {
        match self.with_btc_config(|config| config.network()) {
            BitcoinNetwork::Mainnet => MAINNET_CHAIN_ID,
            BitcoinNetwork::Testnet => TESTNET_CHAIN_ID,
            BitcoinNetwork::Regtest => REGTEST_CHAIN_ID,
        }
    }

    pub fn ck_btc_ledger_fee(&self) -> u64 {
        self.with_btc_config(|config| config.ledger_fee())
    }

    pub fn token_address(&self) -> H160 {
        self.with_wrapped_token_config(|config| config.token_address.clone())
    }

    pub fn token_name(&self) -> [u8; 32] {
        self.with_wrapped_token_config(|config| config.token_name)
    }

    pub fn token_symbol(&self) -> [u8; 16] {
        self.with_wrapped_token_config(|config| config.token_symbol)
    }

    pub fn decimals(&self) -> u8 {
        self.with_wrapped_token_config(|config| config.decimals)
    }

    fn with_btc_config<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&BitcoinConnection) -> T,
    {
        let config = self.btc_config.get();
        f(config)
    }

    fn with_wrapped_token_config<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&WrappedTokenConfig) -> T,
    {
        let config = self.wrapped_token_config.get();
        f(config)
    }
}
