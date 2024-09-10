use candid::{CandidType, Nat};
use did::{H160, H256};
use serde::{Deserialize, Serialize};

use crate::events::{BurntEventData, MintedEventData};
use crate::id256::Id256;
use crate::order::{MintOrder, SignedMintOrder, SignedOrders};
use crate::reason::Icrc2Burn;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum IcrcBridgeOp {
    // Deposit operations:
    BurnIcrc2Tokens(Icrc2Burn),
    SignMintOrder {
        order: MintOrder,
        is_refund: bool,
    },
    SendMintTransaction {
        order: SignedOrders,
        is_refund: bool,
    },
    ConfirmMint {
        order: SignedMintOrder,
        tx_hash: Option<H256>,
        is_refund: bool,
    },
    WrappedTokenMintConfirmed(MintedEventData),

    // Withdraw operations:
    MintIcrcTokens(BurntEventData),
    IcrcMintConfirmed {
        src_address: H160,
        icrc_tx_id: Nat,
    },
}

impl IcrcBridgeOp {
    pub fn get_signed_mint_order(&self, token: &Id256) -> Option<SignedMintOrder> {
        match self {
            Self::SendMintTransaction { order, .. } if &order.get_src_token_id() == token => {
                Some(*order)
            }
            Self::ConfirmMint { order, .. } if &order.get_src_token_id() == token => Some(*order),
            _ => None,
        }
    }
}
