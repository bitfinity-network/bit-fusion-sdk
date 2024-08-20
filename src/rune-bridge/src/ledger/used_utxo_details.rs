use std::borrow::Cow;
use std::str::FromStr as _;

use bitcoin::{Address, Network};
use candid::{CandidType, Decode, Encode};
use ic_stable_structures::{Bound, Storable};
use serde::Deserialize;

#[derive(Debug, Clone, Eq, PartialEq, CandidType, Deserialize)]
pub struct UsedUtxoDetails {
    pub used_at: u64,
    pub owner_address: String,
}

impl UsedUtxoDetails {
    const MAX_BITCOIN_ADDRESS_SIZE: u32 = 96;

    /// Returns the owner address of the utxo.
    pub fn owner_address(&self, network: Network) -> Result<Address, Box<dyn std::error::Error>> {
        Address::from_str(self.owner_address.as_str())?
            .require_network(network)
            .map_err(|e| e.into())
    }
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

    use bitcoin::PublicKey;

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
