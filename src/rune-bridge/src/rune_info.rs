use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use ordinals::{Rune, RuneId};
use serde::{Deserializer, Serialize};

#[derive(Debug, Copy, Clone, CandidType, Deserialize)]
pub struct RuneInfo {
    pub name: RuneName,
    pub decimals: u8,
    pub block: u64,
    pub tx: u32,
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
pub struct RuneName(Rune);

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
