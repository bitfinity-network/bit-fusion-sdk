use std::collections::HashMap;

use candid::CandidType;
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use serde::{Deserialize, Serialize};

use crate::events::MintedEventData;
use crate::op_id::OperationId;
use crate::order::{MintOrder, SignedMintOrder};
use crate::runes::{DidTransaction, RuneName, RuneToWrap, RuneWithdrawalPayload};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone, PartialEq, Eq)]
pub enum RuneBridgeOp {
    // Deposit
    AwaitInputs {
        dst_address: H160,
        dst_tokens: HashMap<RuneName, H160>,
        requested_amounts: Option<HashMap<RuneName, u128>>,
    },
    AwaitConfirmations {
        dst_address: H160,
        utxo: Utxo,
        runes_to_wrap: Vec<RuneToWrap>,
    },
    SignMintOrder {
        dst_address: H160,
        mint_order: MintOrder,
    },
    SendMintOrder {
        dst_address: H160,
        order: SignedMintOrder,
    },
    ConfirmMintOrder {
        dst_address: H160,
        order: SignedMintOrder,
        tx_id: H256,
    },
    MintOrderConfirmed {
        data: MintedEventData,
    },

    // Withdraw
    CreateTransaction {
        payload: RuneWithdrawalPayload,
    },
    SendTransaction {
        from_address: H160,
        transaction: DidTransaction,
    },
    TransactionSent {
        from_address: H160,
        transaction: DidTransaction,
    },

    OperationSplit {
        wallet_address: H160,
        new_operation_ids: Vec<OperationId>,
    },
}
