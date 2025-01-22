use std::collections::HashMap;

use candid::CandidType;
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use serde::{Deserialize, Serialize};

use crate::events::MintedEventData;
use crate::order::{MintOrder, SignedOrders};
use crate::runes::{DidTransaction, RuneName, RuneToWrap, RuneWithdrawalPayload};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone, PartialEq, Eq)]
pub enum RuneBridgeDepositOp {
    /// Await inputs from the Rune deposit provider
    AwaitInputs {
        dst_address: H160,
        dst_tokens: HashMap<RuneName, H160>,
        requested_amounts: Option<HashMap<RuneName, u128>>,
    },
    /// Await confirmations for the deposit
    AwaitConfirmations {
        dst_address: H160,
        utxo: Utxo,
        runes_to_wrap: Vec<RuneToWrap>,
    },
    /// Sign the mint order
    SignMintOrder(MintOrder),
    /// Send the mint order to the bridge
    SendMintOrder(SignedOrders),
    /// Confirm the mint order
    ConfirmMintOrder { order: SignedOrders, tx_id: H256 },
    /// The mint order has been confirmed
    MintOrderConfirmed { data: MintedEventData },
}

#[derive(Debug, Serialize, Deserialize, CandidType, Clone, PartialEq, Eq)]
pub enum RuneBridgeWithdrawOp {
    /// Create a withdrawal transaction
    CreateTransaction { payload: RuneWithdrawalPayload },
    /// Send the withdrawal transaction
    SendTransaction {
        from_address: H160,
        transaction: DidTransaction,
    },
    /// The withdrawal transaction has been sent
    TransactionSent {
        from_address: H160,
        transaction: DidTransaction,
    },
}

#[derive(Debug, Serialize, Deserialize, CandidType, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum RuneBridgeOp {
    Deposit(RuneBridgeDepositOp),
    Withdraw(RuneBridgeWithdrawOp),
}
