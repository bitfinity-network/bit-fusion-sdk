use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

const BRC20_BRIDGE_NAME: &str = "brc20-bridge";
const BTC_BRIDGE_NAME: &str = "btc-bridge";
const ERC20_BRIDGE_NAME: &str = "erc20-bridge";
const ICRC2_BRIDGE_NAME: &str = "icrc2-bridge";
const RUNE_BRIDGE_NAME: &str = "rune-bridge";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Canister {
    Brc20,
    Btc,
    Erc20,
    Icrc2,
    Rune,
}

impl fmt::Display for Canister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let canister = match self {
            Canister::Brc20 => BRC20_BRIDGE_NAME,
            Canister::Btc => BTC_BRIDGE_NAME,
            Canister::Erc20 => ERC20_BRIDGE_NAME,
            Canister::Icrc2 => ICRC2_BRIDGE_NAME,
            Canister::Rune => RUNE_BRIDGE_NAME,
        };

        write!(f, "{}", canister)
    }
}

impl FromStr for Canister {
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

impl Serialize for Canister {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'a> Deserialize<'a> for Canister {
    fn deserialize<D>(deserializer: D) -> Result<Canister, D::Error>
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
        assert_eq!(Canister::Brc20.to_string(), BRC20_BRIDGE_NAME);
        assert_eq!(Canister::Btc.to_string(), BTC_BRIDGE_NAME);
        assert_eq!(Canister::Erc20.to_string(), ERC20_BRIDGE_NAME);
        assert_eq!(Canister::Icrc2.to_string(), ICRC2_BRIDGE_NAME);
        assert_eq!(Canister::Rune.to_string(), RUNE_BRIDGE_NAME);
    }

    #[test]
    fn test_canister_from_str() {
        assert_eq!(Canister::from_str(BRC20_BRIDGE_NAME), Ok(Canister::Brc20));
        assert_eq!(Canister::from_str(BTC_BRIDGE_NAME), Ok(Canister::Btc));
        assert_eq!(Canister::from_str(ERC20_BRIDGE_NAME), Ok(Canister::Erc20));
        assert_eq!(Canister::from_str(ICRC2_BRIDGE_NAME), Ok(Canister::Icrc2));
        assert_eq!(Canister::from_str(RUNE_BRIDGE_NAME), Ok(Canister::Rune));
        assert_eq!(Canister::from_str("invalid"), Err("invalid canister"));
    }
}
