pub mod bitcoin_api;
pub mod inscription;
mod types;
mod utils;

use candid::{CandidType, Deserialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum Error {
    TransactionNotSent(String),
    NoUtxosReturned(String),
    NoBalanceReturned(String),
    CurrentFeePercentilesUnavailable(String),
}

// Enable Candid export
ic_cdk::export_candid!();
