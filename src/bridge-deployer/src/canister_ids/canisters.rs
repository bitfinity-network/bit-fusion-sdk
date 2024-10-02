use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

const BRC20_BRIDGE_NAME: &str = "brc20-bridge";
const BTC_BRIDGE_NAME: &str = "btc-bridge";
const ERC20_BRIDGE_NAME: &str = "erc20-bridge";
const ICRC2_BRIDGE_NAME: &str = "icrc2-bridge";
const RUNE_BRIDGE_NAME: &str = "rune-bridge";

/// Canister type to set the principal for
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CanisterType {
    Brc20,
    Btc,
    Erc20,
    Icrc2,
    Rune,
}

impl fmt::Display for CanisterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let canister = match self {
            CanisterType::Brc20 => BRC20_BRIDGE_NAME,
            CanisterType::Btc => BTC_BRIDGE_NAME,
            CanisterType::Erc20 => ERC20_BRIDGE_NAME,
            CanisterType::Icrc2 => ICRC2_BRIDGE_NAME,
            CanisterType::Rune => RUNE_BRIDGE_NAME,
        };

        write!(f, "{}", canister)
    }
}

impl FromStr for CanisterType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            BRC20_BRIDGE_NAME => Ok(Self::Brc20),
            BTC_BRIDGE_NAME => Ok(Self::Btc),
            ERC20_BRIDGE_NAME => Ok(Self::Erc20),
            ICRC2_BRIDGE_NAME => Ok(Self::Icrc2),
            RUNE_BRIDGE_NAME => Ok(Self::Rune),
            _ => Err("invalid canister"),
        }
    }
}

impl Serialize for CanisterType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'a> Deserialize<'a> for CanisterType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let canister = String::deserialize(deserializer)?;

        Self::from_str(canister.as_str()).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_canister_display() {
        assert_eq!(CanisterType::Brc20.to_string(), BRC20_BRIDGE_NAME);
        assert_eq!(CanisterType::Btc.to_string(), BTC_BRIDGE_NAME);
        assert_eq!(CanisterType::Erc20.to_string(), ERC20_BRIDGE_NAME);
        assert_eq!(CanisterType::Icrc2.to_string(), ICRC2_BRIDGE_NAME);
        assert_eq!(CanisterType::Rune.to_string(), RUNE_BRIDGE_NAME);
    }

    #[test]
    fn test_canister_from_str() {
        assert_eq!(
            CanisterType::from_str(BRC20_BRIDGE_NAME),
            Ok(CanisterType::Brc20)
        );
        assert_eq!(
            CanisterType::from_str(BTC_BRIDGE_NAME),
            Ok(CanisterType::Btc)
        );
        assert_eq!(
            CanisterType::from_str(ERC20_BRIDGE_NAME),
            Ok(CanisterType::Erc20)
        );
        assert_eq!(
            CanisterType::from_str(ICRC2_BRIDGE_NAME),
            Ok(CanisterType::Icrc2)
        );
        assert_eq!(
            CanisterType::from_str(RUNE_BRIDGE_NAME),
            Ok(CanisterType::Rune)
        );
        assert_eq!(CanisterType::from_str("invalid"), Err("invalid canister"));
    }

    #[test]
    fn test_should_serialize() {
        let canister = CanisterType::Brc20;
        let serialized = serde_json::to_string(&canister).unwrap();
        assert_eq!(serialized, format!("\"{}\"", BRC20_BRIDGE_NAME));
    }

    #[test]
    fn test_should_deserialize() {
        let canister = CanisterType::Brc20;
        let serialized = "\"brc20-bridge\"";
        let deserialized: CanisterType = serde_json::from_str(serialized).unwrap();
        assert_eq!(deserialized, canister);
    }
}
