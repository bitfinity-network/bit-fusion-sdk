use candid::CandidType;
use did::H256;
use serde::{Deserialize, Serialize};

use crate::batch_mint_result::BatchMintErrorCode;
use crate::bridge_side::BridgeSide;
use crate::events::MintedEventData;
use crate::order::{MintOrder, SignedOrders};

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
    SendMintTransaction(SignedOrders),
    WaitForMintConfirm {
        order: SignedOrders,
        mint_results: Vec<BatchMintErrorCode>,
        tx_hash: Option<H256>,
    },
    TokenMintConfirmed(MintedEventData),
}

impl Erc20OpStage {
    pub fn name(&self) -> String {
        match self {
            Erc20OpStage::SignMintOrder(_) => String::from("SignMintOrder"),
            Erc20OpStage::SendMintTransaction(_) => String::from("SendMintTransaction"),
            Erc20OpStage::WaitForMintConfirm { .. } => String::from("ConfirmMint"),
            Erc20OpStage::TokenMintConfirmed(_) => String::from("TokenMintConfirmed"),
        }
    }
}
