use bridge_utils::bft_events::BurntEventData;
use bridge_utils::bridge::{self, Operation, OperationContext};
use bridge_utils::operation_store::OperationStore;
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

    async fn progress(self, _ctx: impl OperationContext) -> Result<Self, bridge::Error> {
        todo!()
    }
}
