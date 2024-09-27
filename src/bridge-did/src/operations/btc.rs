use candid::CandidType;
use did::{H160, H256};
use serde::{Deserialize, Serialize};

use crate::events::{BurntEventData, MintedEventData};
use crate::order::{MintOrder, SignedOrders};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum BtcBridgeOp {
    // Deposit operations:
    UpdateCkBtcBalance {
        eth_address: H160,
    },
    CollectCkBtcBalance {
        eth_address: H160,
    },
    TransferCkBtc {
        eth_address: H160,
        amount: u64,
    },
    CreateMintOrder {
        eth_address: H160,
        amount: u64,
    },
    SignMintOrder {
        eth_address: H160,
        order: MintOrder,
    },
    MintErc20 {
        eth_address: H160,
        order: SignedOrders,
    },
    ConfirmErc20Mint {
        order: SignedOrders,
        tx_id: H256,
    },
    Erc20MintConfirmed(MintedEventData),

    // Withdraw operations:
    WithdrawBtc(BurntEventData),
    BtcWithdrawConfirmed {
        eth_address: H160,
    },
}
