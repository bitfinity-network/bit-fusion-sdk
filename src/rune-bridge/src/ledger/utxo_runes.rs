use std::borrow::Cow;

use ic_stable_structures::{Bound, Storable};

use crate::rune_info::RuneInfo;

/// Data structure to keep track rune information for a utxo.
pub struct UtxoRunes(Vec<RuneInfo>);

impl UtxoRunes {
    /// Returns the list of rune information.
    pub fn runes(&self) -> &[RuneInfo] {
        &self.0
    }
}

impl From<Vec<RuneInfo>> for UtxoRunes {
    fn from(runes: Vec<RuneInfo>) -> Self {
        Self(runes)
    }
}

impl Storable for UtxoRunes {
    const BOUND: Bound = Bound::Unbounded;

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let list_len = u64::from_be_bytes(bytes[0..8].try_into().unwrap()) as usize;
        let mut runes = Vec::with_capacity(list_len);

        for rune_buf in bytes[8..bytes.len()].chunks(RuneInfo::BOUND.max_size() as usize) {
            runes.push(RuneInfo::from_bytes(Cow::Borrowed(rune_buf)));
        }

        Self(runes)
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut bytes = Vec::with_capacity(8 + self.0.len() * RuneInfo::BOUND.max_size() as usize);
        bytes.extend_from_slice(&(self.0.len() as u64).to_be_bytes());

        for rune in &self.0 {
            bytes.extend_from_slice(&rune.to_bytes());
        }

        Cow::Owned(bytes)
    }
}

#[cfg(test)]
mod test {

    use ordinals::Rune;

    use super::*;
    use crate::rune_info::RuneName;

    #[test]
    fn test_should_encode_and_decode_utxo_runes() {
        let rune_info = vec![
            RuneInfo {
                name: RuneName::from(Rune(0xdeadbeef)),
                decimals: 18,
                block: 0x1234567890abcdef,
                tx: 0x12345678,
            },
            RuneInfo {
                name: RuneName::from(Rune(0xcafebabe)),
                decimals: 18,
                block: 0x1234567890abcdef,
                tx: 0x12345678,
            },
        ];

        let utxo_runes = UtxoRunes(rune_info.clone());
        let bytes = utxo_runes.to_bytes();
        let decoded = UtxoRunes::from_bytes(Cow::Borrowed(&bytes));

        assert_eq!(utxo_runes.0, decoded.0);
    }
}
