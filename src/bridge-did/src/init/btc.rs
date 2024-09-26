use std::borrow::Cow;

use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_stable_structures::{Bound, Storable};

use crate::init::BridgeInitData;

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
            BitcoinConnection::Mainnet => {
                Principal::from_text("mqygn-kiaaa-aaaar-qaadq-cai").unwrap()
            }
            BitcoinConnection::Testnet => {
                Principal::from_text("ml52i-qqaaa-aaaar-qaaba-cai").unwrap()
            }
            BitcoinConnection::Custom { ckbtc_minter, .. } => *ckbtc_minter,
        }
    }

    pub fn ckbtc_ledger(&self) -> Principal {
        match self {
            BitcoinConnection::Mainnet => {
                Principal::from_text("mxzaz-hqaaa-aaaar-qaada-cai").unwrap()
            }
            BitcoinConnection::Testnet => {
                Principal::from_text("mc6ru-gyaaa-aaaar-qaaaq-cai").unwrap()
            }
            BitcoinConnection::Custom { ckbtc_ledger, .. } => *ckbtc_ledger,
        }
    }

    pub fn ledger_fee(&self) -> u64 {
        match self {
            BitcoinConnection::Mainnet => 10,
            BitcoinConnection::Testnet => 10,
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
