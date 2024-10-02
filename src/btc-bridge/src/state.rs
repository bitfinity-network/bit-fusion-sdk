use bridge_canister::memory::memory_by_id;
use bridge_did::init::BitcoinConnection;
use candid::{CandidType, Principal};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, Storable, VirtualMemory};
use serde::Deserialize;

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

#[derive(Debug, Default, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct WrappedTokenConfig {
    pub token_address: H160,
    pub token_name: [u8; 32],
    pub token_symbol: [u8; 16],
    pub decimals: u8,
}

impl WrappedTokenConfig {
    const MAX_SIZE: u32 = 20 + 32 + 16 + 1;
}

impl Storable for WrappedTokenConfig {
    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Bounded {
        max_size: Self::MAX_SIZE,
        is_fixed_size: false,
    };

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let token_address = H160::from_slice(&bytes[0..20]);
        let token_name = bytes[20..52].try_into().unwrap();
        let token_symbol = bytes[52..68].try_into().unwrap();
        let decimals = bytes[68];

        Self {
            token_address,
            token_name,
            token_symbol,
            decimals,
        }
    }

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let mut bytes = Vec::with_capacity(Self::MAX_SIZE as usize);
        bytes.extend_from_slice(self.token_address.0.as_bytes());
        bytes.extend_from_slice(&self.token_name);
        bytes.extend_from_slice(&self.token_symbol);
        bytes.push(self.decimals);

        bytes.into()
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_should_encode_decode_wrapped_token_config() {
        let config = WrappedTokenConfig {
            token_address: H160::from_slice(&[1; 20]),
            token_name: [1; 32],
            token_symbol: [1; 16],
            decimals: 18,
        };

        let bytes = config.to_bytes();
        let decoded = WrappedTokenConfig::from_bytes(bytes.clone());

        assert_eq!(config, decoded);
    }
}
