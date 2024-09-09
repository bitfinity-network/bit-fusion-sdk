use candid::CandidType;
use did::H160;
use serde::{Deserialize, Serialize};

use super::rune_info::RuneInfo;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuneWithdrawalPayload {
    pub rune_info: RuneInfo,
    pub amount: u128,
    pub request_ts: u64,
    pub sender: H160,
    pub dst_address: String,
}

// impl RuneWithdrawalPayload {
//     pub fn new(burnt_event_data: BurntEventData, state: &RuneState) -> Result<Self, WithdrawError> {
//         let BurntEventData {
//             recipient_id,
//             amount,
//             to_token,
//             sender,
//             ..
//         } = burnt_event_data;

//         let amount = amount.0.as_u128();

//         let Ok(address_string) = String::from_utf8(recipient_id.clone()) else {
//             return Err(WithdrawError::InvalidRequest(format!(
//                 "Failed to decode recipient address from raw data: {recipient_id:?}"
//             )));
//         };

//         let Ok(address) = Address::from_str(&address_string) else {
//             return Err(WithdrawError::InvalidRequest(format!(
//                 "Failed to decode recipient address from string: {address_string}"
//             )));
//         };

//         let Some(token_id) = Id256::from_slice(&to_token) else {
//             return Err(WithdrawError::InvalidRequest(format!(
//                 "Failed to decode token id from the value {to_token:?}"
//             )));
//         };

//         let Ok(rune_id) = token_id.try_into() else {
//             return Err(WithdrawError::InvalidRequest(format!(
//                 "Failed to decode rune id from the token id {to_token:?}"
//             )));
//         };

//         let Some(rune_info) = state.rune_info(rune_id) else {
//             // We don't need to request the list from the indexer at this point. This operation is
//             // called only when some tokens are burned, which means they have been minted before,
//             // and that means that we already received the rune info from the indexer.
//             return Err(WithdrawError::InvalidRequest(format!(
//                 "Invalid rune id: {rune_id}. No such rune id in the rune list received from the indexer."
//             )));
//         };

//         Ok(Self {
//             rune_info,
//             amount,
//             request_ts: ic::time(),
//             sender,
//             dst_address: address.assume_checked().to_string(),
//         })
//     }
// }
