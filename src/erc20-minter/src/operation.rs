use bridge_utils::bft_bridge_api::BurntEventData;
use bridge_utils::evm_bridge::BridgeSide;
use bridge_utils::operation_store::MinterOperation;
use candid::{CandidType, Deserialize};
use did::{H256, U256};
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct OperationPayload {
    pub side: BridgeSide,
    pub status: OperationStatus,
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

#[derive(Debug, Clone, CandidType, Deserialize)]
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

impl MinterOperation for OperationPayload {
    fn is_complete(&self) -> bool {
        matches!(self.status, OperationStatus::Minted { .. })
    }
}
