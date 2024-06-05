use candid::CandidType;
use did::H256;
use minter_did::order::SignedMintOrder;
use serde::Deserialize;

use crate::rune_info::RuneName;

pub mod deposit;
pub mod deposit_store;
pub mod index_provider;
pub mod utxo_provider;
pub mod withdrawal;

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum DepositResult {
    MintOrderSigned {
        mint_order: Box<SignedMintOrder>,
        rune_name: RuneName,
        amount: u128,
    },
    MintRequested {
        tx_id: H256,
        rune_name: RuneName,
        amount: u128,
    },
    Minted {
        tx_id: H256,
        rune_name: RuneName,
        amount: u128,
    },
}
