use std::borrow::Cow;
use std::fmt;

use bitcoin::hashes::sha256d::Hash;
use bitcoin::OutPoint;
use candid::{CandidType, Decode, Encode};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Outpoint;
use ic_stable_structures::{Bound, Storable};
use serde::Deserialize;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, CandidType, Deserialize, Hash)]
pub struct UtxoKey {
    pub tx_id: [u8; 32],
    pub vout: u32,
}

impl fmt::Display for UtxoKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", hex::encode(self.tx_id), self.vout)
    }
}

impl Storable for UtxoKey {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = Encode!(self).expect("cannot serialize utxo key");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("cannot deserialize utxo key")
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 60,
        is_fixed_size: true,
    };
}

impl From<OutPoint> for UtxoKey {
    fn from(value: OutPoint) -> Self {
        Self {
            tx_id: *<Hash as AsRef<[u8; 32]>>::as_ref(&value.txid.to_raw_hash()),
            vout: value.vout,
        }
    }
}

impl From<&Outpoint> for UtxoKey {
    fn from(value: &Outpoint) -> Self {
        Self {
            tx_id: value.txid.clone().try_into().expect("invalid tx id"),
            vout: value.vout,
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_encode_and_decode_key() {
        let key = UtxoKey {
            tx_id: [123; 32],
            vout: 544331,
        };

        let serialized = key.to_bytes();
        let Bound::Bounded { max_size, .. } = UtxoKey::BOUND else {
            panic!("Key is unbounded");
        };

        assert_eq!(serialized.len() as u32, max_size);
        let deserialized = UtxoKey::from_bytes(serialized);
        assert_eq!(deserialized, key);
    }
}
