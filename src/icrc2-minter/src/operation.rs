use bridge_utils::bft_bridge_api::BurntEventData;
use bridge_utils::operation_store::MinterOperation;
use candid::{CandidType, Nat, Principal};
use did::{H256, U256};
use icrc_client::account::Account;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;
use minter_did::reason::Icrc2Burn;
use serde::Deserialize;

use crate::tasks::BurntIcrc2Data;

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum OperationState {
    Deposit(DepositOperationState),
    Withdrawal(WithdrawalOperationState),
}

impl MinterOperation for OperationState {
    fn is_complete(&self) -> bool {
        match self {
            OperationState::Deposit(v) => v.is_complete(),
            OperationState::Withdrawal(v) => v.is_complete(),
        }
    }
}

impl OperationState {
    pub fn new_deposit(data: Icrc2Burn) -> Self {
        Self::Deposit(DepositOperationState::Scheduled(data))
    }

    pub fn new_withdrawal(data: BurntEventData) -> Self {
        Self::Withdrawal(WithdrawalOperationState::Scheduled(data))
    }

    pub fn get_signed_mint_order(&self, for_token: Option<Id256>) -> Option<&SignedMintOrder> {
        match self {
            Self::Deposit(
                DepositOperationState::MintOrderSigned {
                    signed_mint_order,
                    token_id,
                    ..
                }
                | DepositOperationState::MintOrderSent {
                    signed_mint_order,
                    token_id,
                    ..
                },
            ) if for_token.is_none() || matches!(for_token, Some(id) if id == *token_id) => {
                Some(signed_mint_order)
            }
            Self::Withdrawal(WithdrawalOperationState::RefundMintOrderSigned {
                signed_mint_order,
                token_id,
                ..
            }) if for_token.is_none() || matches!(for_token, Some(id) if id == *token_id) => {
                Some(signed_mint_order)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum DepositOperationState {
    Scheduled(Icrc2Burn),
    Icrc2Burned(BurntIcrc2Data),
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

impl DepositOperationState {
    fn is_complete(&self) -> bool {
        matches!(self, Self::Minted { .. })
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum WithdrawalOperationState {
    Scheduled(BurntEventData),
    RefundScheduled(BurntIcrc2Data),
    Transferred {
        token: Principal,
        recipient: Account,
        amount: Nat,
        tx_id: Nat,
    },
    RefundMintOrderSigned {
        token_id: Id256,
        amount: U256,
        signed_mint_order: Box<SignedMintOrder>,
    },
    RefundMintOrderSent {
        token_id: Id256,
        amount: U256,
        signed_mint_order: Box<SignedMintOrder>,
        tx_id: H256,
    },
    RefundMinted {
        token_id: Id256,
        amount: U256,
        tx_id: H256,
    },
}

impl WithdrawalOperationState {
    fn is_complete(&self) -> bool {
        matches!(
            self,
            WithdrawalOperationState::Transferred { .. }
                | WithdrawalOperationState::RefundMinted { .. }
        )
    }
}
