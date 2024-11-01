use std::time::Duration;

use candid::CandidType;
use did::codec;
use eth_signer::sign_strategy::SigningStrategy;
use ic_stable_structures::{Bound, Storable};
use serde::{Deserialize, Serialize};

use crate::evm_link::EvmLink;

#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, CandidType)]
pub struct QueryDelays {
    pub evm_params_query: Duration,
    pub logs_query: Duration,
}

impl Storable for QueryDelays {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        codec::encode(self).into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        codec::decode(bytes.as_ref())
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(Debug, Clone, Deserialize, CandidType)]
pub struct BaseEvmSettings {
    pub evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
    pub delays: QueryDelays,
}
