use candid::CandidType;
use did::H160;
use ic_stable_structures::Storable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WrappedTokenConfig {
    pub token_address: H160,
    pub token_name: [u8; 32],
    pub token_symbol: [u8; 16],
    pub decimals: u8,
}

impl WrappedTokenConfig {
    const MAX_SIZE: u32 = 20 + 32 + 16 + 1;
}

impl Storable for WrappedTokenConfig {
    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Bounded {
        max_size: Self::MAX_SIZE,
        is_fixed_size: false,
    };

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let token_address = H160::from_slice(&bytes[0..20]);
        let token_name = bytes[20..52].try_into().unwrap();
        let token_symbol = bytes[52..68].try_into().unwrap();
        let decimals = bytes[68];

        Self {
            token_address,
            token_name,
            token_symbol,
            decimals,
        }
    }

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let mut bytes = Vec::with_capacity(Self::MAX_SIZE as usize);
        bytes.extend_from_slice(self.token_address.0.as_bytes());
        bytes.extend_from_slice(&self.token_name);
        bytes.extend_from_slice(&self.token_symbol);
        bytes.push(self.decimals);

        bytes.into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_should_encode_decode_wrapped_token_config() {
        let config = WrappedTokenConfig {
            token_address: H160::from_slice(&[1; 20]),
            token_name: [1; 32],
            token_symbol: [1; 16],
            decimals: 18,
        };

        let bytes = config.to_bytes();
        let decoded = WrappedTokenConfig::from_bytes(bytes.clone());

        assert_eq!(config, decoded);
    }
}
