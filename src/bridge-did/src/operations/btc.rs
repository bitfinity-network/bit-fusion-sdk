use candid::CandidType;
use did::H160;
use serde::{Deserialize, Serialize};

use crate::events::{BurntEventData, MintedEventData};
use crate::order::SignedMintOrder;

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
    MintErc20 {
        eth_address: H160,
        order: SignedMintOrder,
    },
    ConfirmErc20Mint {
        order: SignedMintOrder,
        eth_address: H160,
    },
    Erc20MintConfirmed(MintedEventData),

    // Withdraw operations:
    WithdrawBtc(BurntEventData),
    BtcWithdrawConfirmed {
        eth_address: H160,
    },
}
