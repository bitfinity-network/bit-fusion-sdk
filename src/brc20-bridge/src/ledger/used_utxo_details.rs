use std::borrow::Cow;

use candid::{CandidType, Decode, Encode};
use ic_stable_structures::{Bound, Storable};
use serde::Deserialize;

/// Details regarding a used utxo.
#[derive(Debug, Clone, Eq, PartialEq, CandidType, Deserialize)]
pub struct UsedUtxoDetails {
    /// timestamp when the utxo was used.
    pub used_at: u64,
    /// address of the utxo owner.
    pub owner_address: String,
}

impl UsedUtxoDetails {
    const MAX_BITCOIN_ADDRESS_SIZE: u32 = 96;
}

impl Storable for UsedUtxoDetails {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = Encode!(self).expect("failed to serialize utxo");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to deserialize utxo")
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: size_of::<u64>() as u32 + Self::MAX_BITCOIN_ADDRESS_SIZE,
        is_fixed_size: false,
    };
}

#[cfg(test)]
mod test {

    use std::str::FromStr as _;

    use bitcoin::{Address, Network, PublicKey};

    use super::*;

    #[test]
    fn test_should_serialize_used_utxo() {
        let address = Address::p2wpkh(
            &PublicKey::from_str(
                "038f47dcd43ba6d97fc9ed2e3bba09b175a45fac55f0683e8cf771e8ced4572354",
            )
            .unwrap(),
            Network::Signet,
        )
        .unwrap();
        let value = UsedUtxoDetails {
            used_at: 100500,
            owner_address: address.to_string(),
        };

        let serialized = value.to_bytes();
        let Bound::Bounded { max_size, .. } = UsedUtxoDetails::BOUND else {
            panic!("Key is unbounded");
        };

        assert!((serialized.len() as u32) < max_size);
        let deserialized = UsedUtxoDetails::from_bytes(serialized);
        assert_eq!(deserialized, value);
    }
}
