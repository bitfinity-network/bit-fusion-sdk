use std::borrow::Cow;

use candid::CandidType;
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
    /*
       Encoding:
       8                                       // value
       4                                       // script_buf.len
       script_buf                              // script_buf
       4                                       // derivation_path.len
       derivation_path.len * (1 + path.len)    // derivation_path
    */

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buff = Vec::with_capacity(
            8 + 4
                + self.script_buf.len()
                + 4
                + self.derivation_path.len()
                + (self
                    .derivation_path
                    .iter()
                    .map(|path| 1 + path.len())
                    .sum::<usize>()),
        );

        buff.extend_from_slice(&self.value.to_le_bytes());
        buff.extend_from_slice(&(self.script_buf.len() as u32).to_le_bytes());
        buff.extend_from_slice(&self.script_buf);
        buff.extend_from_slice(&(self.derivation_path.len() as u32).to_le_bytes());
        for path in &self.derivation_path {
            buff.push(path.len() as u8);
            buff.extend_from_slice(path);
        }

        buff.into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let mut offset = 0;
        let value =
            u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("invalid value"));
        offset += 8;
        let script_buf_len = u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("invalid script_buf_len"),
        );
        offset += 4;
        let script_buf = bytes[offset..offset + script_buf_len as usize].to_vec();
        offset += script_buf_len as usize;
        let derivation_path_len = u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("invalid derivation_path_len"),
        );
        offset += 4;
        let mut derivation_path = Vec::with_capacity(derivation_path_len as usize);
        for _ in 0..derivation_path_len {
            let path_len = bytes[offset] as usize;
            offset += 1;
            let path = bytes[offset..offset + path_len].to_vec();
            offset += path_len;
            derivation_path.push(path);
        }

        Self {
            value,
            script_buf,
            derivation_path,
        }
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
