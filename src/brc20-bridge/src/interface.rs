use std::cell::RefCell;

use bitcoin::{Network, Transaction, Txid};
use clap::ValueEnum;
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use inscriber::wallet::CanisterWallet;
use serde::{Deserialize, Serialize};

use crate::state::State;

pub mod bridge_api;
pub mod store;

/// Retrieves the Bitcoin address for the given derivation path.
pub(crate) async fn get_deposit_address(
    state: &RefCell<State>,
    eth_address: &H160,
    network: BitcoinNetwork,
) -> String {
    let ecdsa_signer = { state.borrow().ecdsa_signer() };
    CanisterWallet::new(network, ecdsa_signer)
        .get_bitcoin_address(eth_address)
        .await
        .to_string()
}

// To avoid pulling the entire `ord` crate into our dependencies, the following types are
// copied from https://github.com/ordinals/ord

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TransactionHtml {
    pub chain: Chain,
    pub inscription_count: u32,
    pub transaction: Transaction,
    pub txid: Txid,
}

#[derive(Default, ValueEnum, Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Chain {
    #[default]
    #[value(alias("main"))]
    Mainnet,
    #[value(alias("test"))]
    Testnet,
    Signet,
    Regtest,
}

impl From<Chain> for Network {
    fn from(chain: Chain) -> Network {
        match chain {
            Chain::Mainnet => Network::Bitcoin,
            Chain::Testnet => Network::Testnet,
            Chain::Signet => Network::Signet,
            Chain::Regtest => Network::Regtest,
        }
    }
}

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Mainnet => "mainnet",
                Self::Regtest => "regtest",
                Self::Signet => "signet",
                Self::Testnet => "testnet",
            }
        )
    }
}

impl std::str::FromStr for Chain {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "regtest" => Ok(Self::Regtest),
            "signet" => Ok(Self::Signet),
            "testnet" => Ok(Self::Testnet),
            _ => anyhow::bail!("invalid chain `{s}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Brc20TokenResponse {
    pub token: Option<TokenInfo>,
    pub supply: Option<TokenSupply>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TokenInfo {
    pub id: String,
    pub number: u32,
    pub block_height: u32,
    pub tx_id: String,
    pub address: String,
    pub ticker: String,
    pub max_supply: String,
    pub mint_limit: String,
    pub decimals: u8,
    pub deploy_timestamp: u64,
    pub minted_supply: String,
    pub tx_count: u32,
    pub self_mint: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TokenSupply {
    pub max_supply: String,
    pub minted_supply: String,
    pub holders: u32,
}
