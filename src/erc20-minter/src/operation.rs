use bridge_canister::bridge::{Operation, OperationContext};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::BftResult;
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::order::SignedMintOrder;
use bridge_utils::bft_events::BurntEventData;
use bridge_utils::evm_bridge::BridgeSide;
use candid::{CandidType, Deserialize};
use did::{H256, U256};
use serde::Serialize;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct OperationPayload {
    pub side: BridgeSide,
    pub status: OperationStatus,
}

impl Operation for OperationPayload {
    async fn progress(self, _id: OperationId, _ctx: RuntimeState<Self>) -> BftResult<Self> {
        todo!()
    }

    fn is_complete(&self) -> bool {
        matches!(self.status, OperationStatus::Minted { .. })
    }

    fn evm_wallet_address(&self) -> did::H160 {
        todo!()
    }

    async fn on_wrapped_token_minted(
        _ctx: impl OperationContext,
        _event: bridge_utils::bft_events::MintedEventData,
    ) -> Option<bridge_canister::bridge::OperationAction<Self>> {
        todo!()
    }

    async fn on_wrapped_token_burnt(
        _ctx: impl OperationContext,
        _event: BurntEventData,
    ) -> Option<bridge_canister::bridge::OperationAction<Self>> {
        todo!()
    }

    async fn on_minter_notification(
        _ctx: impl OperationContext,
        _event: bridge_utils::bft_events::NotifyMinterEventData,
    ) -> Option<bridge_canister::bridge::OperationAction<Self>> {
        todo!()
    }
}

impl OperationPayload {
    pub fn new(side: BridgeSide, burnt_event_data: BurntEventData) -> Self {
        Self {
            side,
            status: OperationStatus::Scheduled(burnt_event_data),
        }
    }

    pub fn get_signed_mint_order(&self, for_token: Option<Id256>) -> Option<&SignedMintOrder> {
        match &self.status {
            OperationStatus::MintOrderSigned {
                signed_mint_order,
                token_id,
                ..
            }
            | OperationStatus::MintOrderSent {
                signed_mint_order,
                token_id,
                ..
            } if for_token.is_none() || matches!(for_token, Some(id) if id == *token_id) => {
                Some(signed_mint_order)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub enum OperationStatus {
    Scheduled(BurntEventData),
    MintOrderSigned {
        token_id: Id256,
        amount: U256,
        signed_mint_order: Box<SignedMintOrder>,
    },
    MintOrderSent {
        token_id: Id256,
        amount: U256,
        signed_mint_order: Box<SignedMintOrder>,
        tx_id: H256,
    },
    Minted {
        token_id: Id256,
        amount: U256,
        tx_id: H256,
    },
}
