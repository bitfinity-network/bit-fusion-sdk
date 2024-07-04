use bridge_utils::bft_events::BurntEventData;
use bridge_utils::operation_store::{MinterOperation, MinterOperationStore};
use candid::CandidType;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::VirtualMemory;
use serde::Deserialize;

use crate::core::deposit::RuneDepositPayload;
use crate::core::withdrawal::RuneWithdrawalPayload;
use crate::state::State;

pub type RuneOperationStore =
    MinterOperationStore<VirtualMemory<DefaultMemoryImpl>, OperationState>;

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum OperationState {
    Deposit(RuneDepositPayload),
    Withdrawal(RuneWithdrawalPayload),
}

impl OperationState {
    pub fn new_withdrawal(burnt_event_data: BurntEventData, state: &State) -> Self {
        Self::Withdrawal(RuneWithdrawalPayload::new(burnt_event_data, state))
    }
}

impl MinterOperation for OperationState {
    fn is_complete(&self) -> bool {
        match self {
            OperationState::Deposit(v) => v.is_complete(),
            OperationState::Withdrawal(v) => v.is_complete(),
        }
    }
}
