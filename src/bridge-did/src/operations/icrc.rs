use candid::{CandidType, Nat};
use did::{H160, H256};
use serde::{Deserialize, Serialize};

use crate::events::{BurntEventData, MintedEventData};
use crate::order::{MintOrder, SignedOrders};
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
        order: SignedOrders,
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
