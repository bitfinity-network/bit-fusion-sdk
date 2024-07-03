use std::collections::HashMap;

use bridge_utils::bft_bridge_api::NotifyMinterEventData;
use candid::{CandidType, Decode, Deserialize};
use did::H160;

use crate::rune_info::RuneName;

#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct RuneDepositRequest {
    pub eth_dst_address: H160,
    pub amounts: Option<HashMap<RuneName, u128>>,
}

impl TryFrom<NotifyMinterEventData> for RuneDepositRequest {
    type Error = candid::Error;

    fn try_from(value: NotifyMinterEventData) -> Result<Self, Self::Error> {
        Decode!(&value.user_data, RuneDepositRequest)
    }
}
