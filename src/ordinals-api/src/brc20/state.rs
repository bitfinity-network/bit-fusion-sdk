use candid::CandidType;
use serde::{Deserialize, Serialize};

/// Provides general `BRC-20` token info(state).
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct Brc20Token {
    pub id: String,
    pub number: u64,
    pub block_height: u64,
    pub tx_id: String,
    pub address: String,
    pub ticker: String,
    pub max_supply: String,
    pub mint_limit: String,
    pub decimals: u64,
    pub deploy_timestamp: i64,
    pub minted_supply: String,
    pub tx_count: u64,
}

impl Brc20Token {
    pub fn get_mint_limit(&self) -> f64 {
        self.mint_limit
            .parse::<f64>()
            .expect("Invalid value or overflow")
    }
}

/// Contains `BRC-20` supply data.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct Brc20Supply {
    pub max_supply: String,
    pub minted_supply: String,
    pub holders: u64,
}

impl Brc20Supply {
    pub fn get_max_supply(&self) -> f64 {
        self.max_supply
            .parse::<f64>()
            .expect("Invalid value or overflow")
    }

    pub fn get_minted_supply(&self) -> f64 {
        self.minted_supply
            .parse::<f64>()
            .expect("Invalid value or overflow")
    }
}

/// Contains `BRC-20` address(holder) balance data.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct Brc20Balance {
    pub ticker: String,
    pub available_balance: String,
    pub transferrable_balance: String,
    pub overall_balance: String,
}

impl Brc20Balance {
    pub fn get_available_balance(&self) -> f64 {
        self.available_balance
            .parse::<f64>()
            .expect("Invalid value or overflow")
    }

    pub fn get_transferrable_balance(&self) -> f64 {
        self.transferrable_balance
            .parse::<f64>()
            .expect("Invalid value or overflow")
    }

    pub fn get_overall_balance(&self) -> f64 {
        self.overall_balance
            .parse::<f64>()
            .expect("Invalid value or overflow")
    }
}

/// `BRC-20` schema api wrapper around supply and token info.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct Brc20TokenDetails {
    pub token: Brc20Token,
    pub supply: Brc20Supply,
}

/// `BRC-20` general token holder info.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct Brc20Holder {
    pub address: String,
    pub overall_balance: String,
}

impl Brc20Holder {
    pub fn get_overall_balance(&self) -> f64 {
        self.overall_balance
            .parse::<f64>()
            .expect("Invalid value or overflow")
    }
}
