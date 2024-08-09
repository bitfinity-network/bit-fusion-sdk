use bridge_canister::bridge::{Operation, OperationContext};
use bridge_canister::operation_store::OperationStore;
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::BftResult;
use bridge_did::op_id::OperationId;
use bridge_utils::bft_events::BurntEventData;
use candid::CandidType;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::VirtualMemory;
use serde::{Deserialize, Serialize};

use crate::core::deposit::RuneDepositPayload;
use crate::core::withdrawal::RuneWithdrawalPayload;
use crate::state::State;

pub type RuneOperationStore = OperationStore<VirtualMemory<DefaultMemoryImpl>, OperationState>;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub enum OperationState {
    Deposit(RuneDepositPayload),
    Withdrawal(RuneWithdrawalPayload),
}

impl OperationState {
    pub fn new_withdrawal(burnt_event_data: BurntEventData, state: &State) -> Self {
        Self::Withdrawal(RuneWithdrawalPayload::new(burnt_event_data, state))
    }
}

impl Operation for OperationState {
    fn is_complete(&self) -> bool {
        match self {
            OperationState::Deposit(v) => v.is_complete(),
            OperationState::Withdrawal(v) => v.is_complete(),
        }
    }

    async fn progress(self, _id: OperationId, _ctx: RuntimeState<Self>) -> BftResult<Self> {
        todo!()
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
