use crate::ck_btc_interface::{PendingUtxo, UpdateBalanceError};
use candid::CandidType;
use did::H256;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use minter_did::order::SignedMintOrder;
use serde::Deserialize;

#[derive(Debug, CandidType, Deserialize)]
pub enum Erc20MintStatus {
    Scheduled {
        current_confirmations: u32,
        required_confirmations: u32,
        pending_utxos: Option<Vec<PendingUtxo>>,
    },
    Signed(SignedMintOrder),
    Minted {
        amount: u64,
        tx_id: H256,
    },
}

#[derive(Debug, CandidType, Deserialize)]
pub enum Erc20MintError {
    ValueTooSmall(Utxo),
    Tainted(Utxo),
    CkBtcError(UpdateBalanceError),
    Sign(String),
    Evm(String),
    NotInitialized,
}
