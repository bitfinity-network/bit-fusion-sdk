use candid::CandidType;
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use serde::{Deserialize, Serialize};

use crate::brc20_info::Brc20Tick;
use crate::events::MintedEventData;
use crate::order::MintOrder;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct DepositRequest {
    pub amount: u128,
    pub brc20_tick: Brc20Tick,
    pub dst_address: H160,
    pub dst_token: H160,
}

/// BRC20 bridge operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeOp {
    /// Deposit operations
    Deposit(Brc20BridgeDepositOp),
}

/// BRC20 bridge deposit operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeDepositOp {
    /// Await for deposit inputs
    AwaitInputs(DepositRequest),
    /// Await for minimum IC confirmations
    AwaitConfirmations {
        deposit: DepositRequest,
        utxos: Vec<Utxo>,
    },
    /// Sign the provided mint order
    SignMintOrder(MintOrder),
    /// Send the signed mint order to the bridge
    SendMintOrder(SignedOrders),
    /// Confirm the mint order
    ConfirmMintOrder {
        signed_mint_order: SignedMintOrder,
        tx_id: H256,
    },
    /// Mint order confirmed status
    MintOrderConfirmed { data: MintedEventData },
}
