use candid::CandidType;
use did::H256;
use serde::{Deserialize, Serialize};

use crate::bridge_side::BridgeSide;
use crate::events::MintedEventData;
use crate::order::{MintOrder, SignedMintOrder};

/// Erc20 bridge operation.
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct Erc20BridgeOp {
    /// Side of the bridge to perform the operation.
    pub side: BridgeSide,

    /// Stage of the operation.
    pub stage: Erc20OpStage,
}

/// Erc20 bridge operation stages.
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Erc20OpStage {
    SignMintOrder(MintOrder),
    SendMintTransaction(SignedMintOrder),
    ConfirmMint {
        order: SignedMintOrder,
        tx_hash: Option<H256>,
    },
    TokenMintConfirmed(MintedEventData),
}
