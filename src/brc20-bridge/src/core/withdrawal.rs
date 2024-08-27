use std::str::FromStr;

use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::{Address, Transaction};
use bridge_did::id256::Id256;
use bridge_utils::bft_events::BurntEventData;
use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use did::H160;
use ic_exports::ic_kit::ic;
use serde::{Deserializer, Serialize};

use crate::brc20_info::{Brc20Info, Brc20Tick};
use crate::interface::WithdrawError;
use crate::state::Brc20State;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct Brc20WithdrawalPayload {
    pub brc20_info: Brc20Info,
    pub amount: u128,
    pub request_ts: u64,
    pub sender: H160,
    pub dst_address: String,
}

impl Brc20WithdrawalPayload {
    pub fn new(
        burnt_event_data: BurntEventData,
        state: &Brc20State,
    ) -> Result<Self, WithdrawError> {
        let BurntEventData {
            recipient_id,
            amount,
            to_token,
            sender,
            ..
        } = burnt_event_data;

        let amount = amount.0.as_u128();

        let Ok(address_string) = String::from_utf8(recipient_id.clone()) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode recipient address from raw data: {recipient_id:?}"
            )));
        };

        let Ok(address) = Address::from_str(&address_string) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode recipient address from string: {address_string}"
            )));
        };

        let Some(token_id) = Id256::from_slice(&to_token) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode token id from the value {to_token:?}"
            )));
        };

        let brc20_tick = Brc20Tick::from(token_id);

        let Some(brc20_info) = state.brc20_info(&brc20_tick) else {
            // We don't need to request the list from the indexer at this point. This operation is
            // called only when some tokens are burned, which means they have been minted before,
            // and that means that we already received the rune info from the indexer.
            return Err(WithdrawError::InvalidRequest(format!(
                "Invalid rune id: {brc20_tick}. No such rune id in the rune list received from the indexer."
            )));
        };

        Ok(Self {
            amount,
            brc20_info,
            request_ts: ic::time(),
            sender,
            dst_address: address.assume_checked().to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DidTransaction(Transaction);

impl CandidType for DidTransaction {
    fn _ty() -> Type {
        <Vec<u8> as CandidType>::_ty()
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        let mut bytes = vec![];
        self.0.consensus_encode(&mut bytes).map_err(Error::custom)?;

        bytes.idl_serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DidTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <Vec<u8> as Deserialize<'de>>::deserialize(deserializer)?;
        let tx =
            Transaction::consensus_decode(&mut &bytes[..]).map_err(serde::de::Error::custom)?;

        Ok(Self(tx))
    }
}

impl Serialize for DidTransaction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::Error;

        let mut bytes = vec![];
        self.0.consensus_encode(&mut bytes).map_err(Error::custom)?;
        serializer.serialize_bytes(&bytes)
    }
}

impl From<Transaction> for DidTransaction {
    fn from(value: Transaction) -> Self {
        Self(value)
    }
}

impl From<DidTransaction> for Transaction {
    fn from(value: DidTransaction) -> Self {
        value.0
    }
}
