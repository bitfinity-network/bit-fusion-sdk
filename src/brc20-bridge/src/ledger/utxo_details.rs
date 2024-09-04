use std::borrow::Cow;

use candid::{CandidType, Decode, Encode};
use ic_stable_structures::{Bound, Storable};
use serde::Deserialize;

/// Utxo details to be stored in the ledger.
#[derive(Debug, Clone, Eq, PartialEq, CandidType, Deserialize)]
pub struct UtxoDetails {
    /// btc value of the utxo.
    pub value: u64,
    /// script buffer of the utxo.
    pub script_buf: Vec<u8>,
    /// derivation path of the utxo.
    pub derivation_path: Vec<Vec<u8>>,
}

impl Storable for UtxoDetails {
    fn to_bytes(&self) -> Cow<[u8]> {
        Encode!(self).expect("Failed to encode UtxoDetails").into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, UtxoDetails).expect("Failed to decode UtxoDetails")
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use bitcoin::{Address, Network, PublicKey};
    use did::H160;

    use super::*;
    use crate::key::get_derivation_path_ic;

    #[test]
    fn test_should_serialize_and_deserialize_details() {
        let address = Address::p2wpkh(
            &PublicKey::from_str(
                "038f47dcd43ba6d97fc9ed2e3bba09b175a45fac55f0683e8cf771e8ced4572354",
            )
            .unwrap(),
            Network::Signet,
        )
        .unwrap();
        let derivation_path = get_derivation_path_ic(
            &H160::from_hex_str("0x0dc9f6938e9b47fd8553df50bcbdb62d67239007").unwrap(),
        );
        let value = UtxoDetails {
            value: 100500,
            script_buf: address.script_pubkey().to_bytes(),
            derivation_path,
        };

        let serialized = value.to_bytes();

        let deserialized = UtxoDetails::from_bytes(serialized);
        assert_eq!(deserialized, value);
    }
}
