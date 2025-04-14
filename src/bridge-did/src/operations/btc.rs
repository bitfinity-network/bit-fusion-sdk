use candid::CandidType;
use did::{H160, H256};
use serde::{Deserialize, Serialize};

use crate::batch_mint_result::BatchMintErrorCode;
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
        order: MintOrder,
    },
    MintErc20 {
        order: SignedOrders,
    },
    WaitForErc20MintConfirm {
        order: SignedOrders,
        mint_result: Vec<BatchMintErrorCode>,
        tx_id: Option<H256>,
    },
    Erc20MintConfirmed(MintedEventData),

    // Withdraw operations:
    WithdrawBtc(BurntEventData),
    BtcWithdrawConfirmed {
        eth_address: H160,
    },
}
