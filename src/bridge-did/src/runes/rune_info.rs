use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use did::H160;
use ic_stable_structures::{Bound, Storable};
use ordinals::{Rune, RuneId};
use serde::{Deserializer, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RuneInfo {
    pub name: RuneName,
    pub decimals: u8,
    pub block: u64,
    pub tx: u32,
}

impl Storable for RuneInfo {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = Vec::with_capacity(Self::BOUND.max_size() as usize);
        buf.extend_from_slice(&self.name.inner().0.to_le_bytes());
        buf.extend_from_slice(&self.decimals.to_le_bytes());
        buf.extend_from_slice(&self.block.to_le_bytes());
        buf.extend_from_slice(&self.tx.to_le_bytes());

        buf.into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let name = u128::from_le_bytes(bytes[0..16].try_into().unwrap());
        let decimals = u8::from_le_bytes(bytes[16..17].try_into().unwrap());
        let block = u64::from_le_bytes(bytes[17..25].try_into().unwrap());
        let tx = u32::from_le_bytes(bytes[25..29].try_into().unwrap());
        Self {
            name: RuneName(Rune(name)),
            decimals,
            block,
            tx,
        }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: size_of::<u128>() as u32
            + size_of::<u8>() as u32
            + size_of::<u64>() as u32
            + size_of::<u32>() as u32,
        is_fixed_size: true,
    };
}

impl RuneInfo {
    pub fn id(&self) -> RuneId {
        RuneId {
            block: self.block,
            tx: self.tx,
        }
    }

    pub fn name(&self) -> RuneName {
        self.name
    }

    pub fn name_array(&self) -> [u8; 32] {
        let mut value = [0; 32];
        let name = self.name.to_string();
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(32);
        value[0..len].copy_from_slice(&name_bytes[0..len]);
        value
    }

    pub fn symbol_array(&self) -> [u8; 16] {
        let mut value = [0; 16];
        let name = self.name.to_string();
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(16);
        value[0..len].copy_from_slice(&name_bytes[0..len]);
        value
    }

    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    pub fn invalid() -> Self {
        Self {
            name: RuneName(Rune(0)),
            decimals: 0,
            block: 0,
            tx: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuneName(pub Rune);

impl RuneName {
    pub fn inner(&self) -> Rune {
        self.0
    }
}

impl FromStr for RuneName {
    type Err = <Rune as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Rune::from_str(s)?))
    }
}

impl Hash for RuneName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0 .0.hash(state)
    }
}

impl From<Rune> for RuneName {
    fn from(value: Rune) -> Self {
        Self(value)
    }
}

impl From<RuneName> for Rune {
    fn from(value: RuneName) -> Self {
        value.0
    }
}

impl Display for RuneName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl CandidType for RuneName {
    fn _ty() -> Type {
        u128::_ty()
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer,
    {
        self.0 .0.idl_serialize(serializer)
    }
}

impl Serialize for RuneName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0 .0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuneName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u128::deserialize(deserializer)?;
        Ok(Self(Rune(value)))
    }
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuneToWrap {
    pub rune_info: RuneInfo,
    pub amount: u128,
    pub wrapped_address: H160,
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_encode_decode_rune_info() {
        let rune_info = RuneInfo {
            name: RuneName(Rune(0x1234567890abcdef)),
            decimals: 18,
            block: 0x1234567890abcdef,
            tx: 0x12345678,
        };

        let bytes = rune_info.to_bytes();
        let decoded = RuneInfo::from_bytes(bytes);

        assert_eq!(rune_info, decoded);
    }
}
