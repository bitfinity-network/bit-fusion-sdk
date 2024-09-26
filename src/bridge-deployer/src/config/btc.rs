use bridge_did::init::BitcoinConnection;
use candid::{Deserialize, Principal};
use clap::{Parser, ValueEnum};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use serde::Serialize;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct BtcBridgeConnection {
    /// Bitcoin network to connect to.
    ///
    /// If regtest is specified, `--ledger`, `--minter` and `--fee` arguments must also be provided.
    #[arg(long)]
    network: BtcNetwork,
    /// ckBTC ledger canister principal.
    #[arg(long)]
    ledger: Option<Principal>,
    /// ckBTC minter canister principal.
    #[arg(long)]
    minter: Option<Principal>,
    /// ckBTC ledger fee in satoshi.
    #[arg(long)]
    fee: Option<u64>,
}

#[derive(ValueEnum, Debug, Serialize, Deserialize, Clone)]
pub enum BtcNetwork {
    Mainnet,
    Testnet,
    Regtest,
}

impl From<BtcNetwork> for BitcoinNetwork {
    fn from(value: BtcNetwork) -> Self {
        match value {
            BtcNetwork::Mainnet => BitcoinNetwork::Mainnet,
            BtcNetwork::Testnet => BitcoinNetwork::Testnet,
            BtcNetwork::Regtest => BitcoinNetwork::Regtest,
        }
    }
}

impl From<BtcBridgeConnection> for BitcoinConnection {
    fn from(value: BtcBridgeConnection) -> Self {
        if value.ledger.is_some() {
            let ledger = value.ledger.expect(
                "either all or none of the `ledger`, `minter`, `fee` arguments must be provided",
            );
            let minter = value.minter.expect(
                "either all or none of the `ledger`, `minter`, `fee` arguments must be provided",
            );
            let fee = value.fee.expect(
                "either all or none of the `ledger`, `minter`, `fee` arguments must be provided",
            );
            BitcoinConnection::Custom {
                network: value.network.into(),
                ckbtc_minter: minter,
                ckbtc_ledger: ledger,
                ledger_fee: fee,
            }
        } else if value.minter.is_some() || value.fee.is_some() {
            panic!(
                "either all or none of the `ledger`, `minter`, `fee` arguments must be provided",
            );
        } else {
            match value.network {
                BtcNetwork::Mainnet => BitcoinConnection::Mainnet,
                BtcNetwork::Testnet => BitcoinConnection::Testnet,
                BtcNetwork::Regtest => panic!("you must provide `ledger`, `minter` and `fee` arguments for regtest connection"),
            }
        }
    }
}
