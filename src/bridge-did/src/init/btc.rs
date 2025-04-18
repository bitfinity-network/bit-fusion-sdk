use std::borrow::Cow;

use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::{Bound, Storable};
use serde::Serialize;

use crate::init::BridgeInitData;

#[derive(Debug, Default, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
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
        bytes.extend_from_slice(self.token_address.0.as_slice());
        bytes.extend_from_slice(&self.token_name);
        bytes.extend_from_slice(&self.token_symbol);
        bytes.push(self.decimals);

        bytes.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct BtcBridgeConfig {
    pub network: BitcoinConnection,
    pub init_data: BridgeInitData,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub enum BitcoinConnection {
    #[default]
    Mainnet,
    Testnet,
    Custom {
        network: BitcoinNetwork,
        ckbtc_minter: Principal,
        ckbtc_ledger: Principal,
        ledger_fee: u64,
    },
}

// https://internetcomputer.org/docs/current/developer-docs/multi-chain/chain-key-tokens/ckbtc/overview#how-it-works

const MAINNET_CKBTC_MINTER: &str = "mqygn-kiaaa-aaaar-qaadq-cai";
const TESTNET_CKBTC_MINTER: &str = "ml52i-qqaaa-aaaar-qaaba-cai";

const MAINNET_CKBTC_LEDGER: &str = "mxzaz-hqaaa-aaaar-qaada-cai";
const TESTNET_CKBTC_LEDGER: &str = "mc6ru-gyaaa-aaaar-qaaaq-cai";

const CKBTC_TRANSFER_FEE: u64 = 10;

impl BitcoinConnection {
    pub fn network(&self) -> BitcoinNetwork {
        match self {
            BitcoinConnection::Mainnet => BitcoinNetwork::Mainnet,
            BitcoinConnection::Testnet => BitcoinNetwork::Testnet,
            BitcoinConnection::Custom { network, .. } => *network,
        }
    }
    pub fn ckbtc_minter(&self) -> Principal {
        match self {
            BitcoinConnection::Mainnet => Principal::from_text(MAINNET_CKBTC_MINTER).unwrap(),
            BitcoinConnection::Testnet => Principal::from_text(TESTNET_CKBTC_MINTER).unwrap(),
            BitcoinConnection::Custom { ckbtc_minter, .. } => *ckbtc_minter,
        }
    }

    pub fn ckbtc_ledger(&self) -> Principal {
        match self {
            BitcoinConnection::Mainnet => Principal::from_text(MAINNET_CKBTC_LEDGER).unwrap(),
            BitcoinConnection::Testnet => Principal::from_text(TESTNET_CKBTC_LEDGER).unwrap(),
            BitcoinConnection::Custom { ckbtc_ledger, .. } => *ckbtc_ledger,
        }
    }

    pub fn ledger_fee(&self) -> u64 {
        match self {
            BitcoinConnection::Mainnet => CKBTC_TRANSFER_FEE,
            BitcoinConnection::Testnet => CKBTC_TRANSFER_FEE,
            BitcoinConnection::Custom { ledger_fee, .. } => *ledger_fee,
        }
    }
}

impl Storable for BitcoinConnection {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).expect("failed to encode bitcoin connection configuration"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode bitcoin connection configuration")
    }

    const BOUND: Bound = Bound::Unbounded;
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
