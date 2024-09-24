use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use candid::{CandidType, Deserialize};
use ic_stable_structures::{Bound, Storable};
use serde::Serialize;

use crate::id256::Id256;

/// Brc20Tick is a 4 bytes ASCII identifier for a BRC20 token.
#[derive(Debug, Copy, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize, Hash)]
pub struct Brc20Tick([u8; 4]);

impl Brc20Tick {
    pub fn inner(&self) -> [u8; 4] {
        self.0
    }

    pub fn name_array(&self) -> [u8; 32] {
        let mut name = [0u8; 32];
        name[0..4].copy_from_slice(&self.0);
        name
    }

    pub fn symbol_array(&self) -> [u8; 16] {
        let mut name = [0u8; 16];
        name[0..4].copy_from_slice(&self.0);
        name
    }
}

impl From<[u8; 4]> for Brc20Tick {
    fn from(tick: [u8; 4]) -> Self {
        Brc20Tick(tick)
    }
}

impl From<Id256> for Brc20Tick {
    fn from(id: Id256) -> Self {
        Brc20Tick(id.to_brc20_tick().expect("unexpected id256"))
    }
}

impl From<Brc20Tick> for Id256 {
    fn from(tick: Brc20Tick) -> Self {
        Id256::from_brc20_tick(tick.inner())
    }
}

impl FromStr for Brc20Tick {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 4 {
            return Err(());
        }

        let mut tick = [0u8; 4];
        tick.copy_from_slice(s.as_bytes());
        Ok(Brc20Tick(tick))
    }
}

impl Display for Brc20Tick {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

/// Brc20 token information.
#[derive(Debug, Copy, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct Brc20Info {
    pub tick: Brc20Tick,
    pub decimals: u8,
}

impl Storable for Brc20Info {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = Vec::with_capacity(Self::BOUND.max_size() as usize);
        buf.extend_from_slice(&self.tick.inner());
        buf.extend_from_slice(&self.decimals.to_le_bytes());

        buf.into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let name = bytes[0..4].try_into().unwrap();
        let decimals = u8::from_le_bytes(bytes[4..5].try_into().unwrap());
        Self {
            tick: Brc20Tick(name),
            decimals,
        }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: size_of::<u32>() as u32 + size_of::<u8>() as u32,
        is_fixed_size: true,
    };
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_encode_decode_brc20_info() {
        let info = Brc20Info {
            tick: Brc20Tick::from_str("ordi").unwrap(),
            decimals: 18,
        };

        let bytes = info.to_bytes();
        let decoded = Brc20Info::from_bytes(bytes);

        assert_eq!(info, decoded);
    }

    #[test]
    fn test_brc20_tick_display() {
        let tick = Brc20Tick::from_str("ordi").unwrap();
        assert_eq!(tick.to_string(), "ordi");
    }

    #[test]
    fn test_brc20_tick_from_str() {
        let tick = Brc20Tick::from_str("ordi").unwrap();
        assert_eq!(tick.inner(), [b'o', b'r', b'd', b'i']);
    }

    #[test]
    fn test_brc20_tick_from_id256() {
        let tick = Brc20Tick::from_str("ordi").unwrap();
        let id256 = Id256::from(tick);
        let tick_from_id256 = Brc20Tick::from(id256);

        assert_eq!(tick, tick_from_id256);
    }

    #[test]
    fn test_brc20_tick_from_str_fail() {
        let tick = Brc20Tick::from_str("ordi1");
        assert!(tick.is_err());
    }

    #[test]
    fn test_brc20_tick_from_str_fail2() {
        let tick = Brc20Tick::from_str("ord");
        assert!(tick.is_err());
    }

    #[test]
    fn test_brc20_tick_name_array() {
        let tick = Brc20Tick::from_str("ordi").unwrap();
        assert_eq!(
            tick.name_array(),
            [
                b'o', b'r', b'd', b'i', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
    }

    #[test]
    fn test_brc20_tick_symbol_array() {
        let tick = Brc20Tick::from_str("ordi").unwrap();
        assert_eq!(
            tick.symbol_array(),
            [b'o', b'r', b'd', b'i', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }
}
