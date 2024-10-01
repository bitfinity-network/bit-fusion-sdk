use bridge_canister::memory::memory_by_id;
use bridge_did::init::btc::WrappedTokenConfig;
use candid::{CandidType, Principal};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, Storable, VirtualMemory};
use serde::Deserialize;

use crate::memory::{BTC_CONFIG_MEMORY_ID, WRAPPED_TOKEN_CONFIG_MEMORY_ID};
use crate::{MAINNET_CHAIN_ID, REGTEST_CHAIN_ID, TESTNET_CHAIN_ID};

pub struct State {
    pub btc_config: StableCell<BtcConfig, VirtualMemory<DefaultMemoryImpl>>,
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
            btc_config: StableCell::new(memory_by_id(BTC_CONFIG_MEMORY_ID), BtcConfig::default())
                .expect("stable memory config initialization failed"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CandidType, Deserialize)]
pub struct BtcConfig {
    pub ck_btc_minter: Principal,
    pub ck_btc_ledger: Principal,
    pub network: BitcoinNetwork,
    pub ck_btc_ledger_fee: u64,
}

impl BtcConfig {
    const MAX_SIZE: u32 = 1 // principal length
        + Principal::MAX_LENGTH_IN_BYTES as u32
        + 1 // principal length
        + Principal::MAX_LENGTH_IN_BYTES as u32
        + 1 // network
        + 8 // fee
        ;

    fn encode_network(&self) -> u8 {
        match self.network {
            BitcoinNetwork::Mainnet => 0,
            BitcoinNetwork::Testnet => 1,
            BitcoinNetwork::Regtest => 2,
        }
    }

    fn decode_network(network: u8) -> BitcoinNetwork {
        match network {
            0 => BitcoinNetwork::Mainnet,
            1 => BitcoinNetwork::Testnet,
            2 => BitcoinNetwork::Regtest,
            _ => panic!("invalid network"),
        }
    }
}

impl Default for BtcConfig {
    fn default() -> Self {
        Self {
            ck_btc_minter: Principal::anonymous(),
            ck_btc_ledger: Principal::anonymous(),
            network: BitcoinNetwork::Regtest,
            ck_btc_ledger_fee: 10,
        }
    }
}

impl Storable for BtcConfig {
    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Bounded {
        max_size: Self::MAX_SIZE,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let ck_btc_minter = self.ck_btc_minter.as_slice().to_vec();
        let ck_btc_ledger = self.ck_btc_ledger.as_slice().to_vec();

        // encode
        // {ck_btc_minter_len}{ck_btc_minter}{ck_btc_ledger_len}{ck_btc_ledger}{network}{fee}
        let mut bytes = Vec::with_capacity(Self::MAX_SIZE as usize);
        bytes.push(ck_btc_minter.len() as u8);
        bytes.extend_from_slice(&ck_btc_minter);
        bytes.push(ck_btc_ledger.len() as u8);
        bytes.extend_from_slice(&ck_btc_ledger);
        bytes.push(self.encode_network());
        bytes.extend_from_slice(&self.ck_btc_ledger_fee.to_le_bytes());

        bytes.into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let mut offset = 0;

        let ck_btc_minter_len = bytes[offset] as usize;
        offset += 1;
        let ck_btc_minter = Principal::from_slice(&bytes[offset..offset + ck_btc_minter_len]);
        offset += ck_btc_minter_len;

        let ck_btc_ledger_len = bytes[offset] as usize;
        offset += 1;
        let ck_btc_ledger = Principal::from_slice(&bytes[offset..offset + ck_btc_ledger_len]);
        offset += ck_btc_ledger_len;

        let network = bytes[offset];
        offset += 1;

        let ck_btc_ledger_fee = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());

        Self {
            ck_btc_minter,
            ck_btc_ledger,
            network: Self::decode_network(network),
            ck_btc_ledger_fee,
        }
    }
}

impl State {
    pub fn configure_btc(&mut self, config: BtcConfig) {
        self.btc_config.set(config).expect("failed to set config");
    }

    pub fn configure_wrapped_token(&mut self, config: WrappedTokenConfig) {
        self.wrapped_token_config
            .set(config)
            .expect("failed to set wrapped token config");
    }

    pub fn ck_btc_minter(&self) -> Principal {
        self.with_btc_config(|config| config.ck_btc_minter)
    }

    pub fn ck_btc_ledger(&self) -> Principal {
        self.with_btc_config(|config| config.ck_btc_ledger)
    }

    pub fn btc_chain_id(&self) -> u32 {
        match self.with_btc_config(|config| config.network) {
            BitcoinNetwork::Mainnet => MAINNET_CHAIN_ID,
            BitcoinNetwork::Testnet => TESTNET_CHAIN_ID,
            BitcoinNetwork::Regtest => REGTEST_CHAIN_ID,
        }
    }

    pub fn ck_btc_ledger_fee(&self) -> u64 {
        self.with_btc_config(|config| config.ck_btc_ledger_fee)
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
        F: FnOnce(&BtcConfig) -> T,
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
    fn test_should_encode_decode_btc_config() {
        let config = BtcConfig {
            ck_btc_minter: Principal::from_slice(&[1; 29]),
            ck_btc_ledger: Principal::from_slice(&[2; 29]),
            network: BitcoinNetwork::Mainnet,
            ck_btc_ledger_fee: 10,
        };

        let bytes = config.to_bytes();
        let decoded = BtcConfig::from_bytes(bytes.clone());

        assert_eq!(config, decoded);
    }

    #[test]
    fn test_should_encode_decode_btc_config_shorter_principal() {
        let config = BtcConfig {
            ck_btc_minter: Principal::from_text("aaaaa-aa").unwrap(),
            ck_btc_ledger: Principal::from_text("aaaaa-aa").unwrap(),
            network: BitcoinNetwork::Mainnet,
            ck_btc_ledger_fee: 10,
        };

        let bytes = config.to_bytes();
        let decoded = BtcConfig::from_bytes(bytes.clone());

        assert_eq!(config, decoded);
    }
}
