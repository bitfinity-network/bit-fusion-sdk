use std::borrow::Cow;
use std::fmt;

use bitcoin::hashes::sha256d::Hash;
use bitcoin::OutPoint;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Outpoint;
use ic_stable_structures::{Bound, Storable};

/// Unique identifier for a utxo.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UtxoKey {
    /// Transaction id of the utxo.
    pub tx_id: [u8; 32],
    /// Vout of the utxo. (Index of the output in the transaction)
    pub vout: u32,
}

impl fmt::Display for UtxoKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", hex::encode(self.tx_id), self.vout)
    }
}

impl Storable for UtxoKey {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buff = Vec::with_capacity(Self::BOUND.max_size() as usize);
        buff.extend_from_slice(&self.tx_id);
        buff.extend_from_slice(&self.vout.to_le_bytes());

        buff.into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let tx_id = bytes[..32].try_into().expect("invalid tx id");
        let vout = u32::from_le_bytes(bytes[32..].try_into().expect("invalid vout"));

        Self { tx_id, vout }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 32 + 4,
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
