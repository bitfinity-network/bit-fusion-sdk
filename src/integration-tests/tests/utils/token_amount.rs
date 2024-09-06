use std::fmt::{self, Display, Formatter};
use std::ops::{Add, Sub};

use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenAmount {
    amount: u128,
    decimals: u8,
}

impl TokenAmount {
    /// Create a new TokenAmount from the given absolute amount and decimals.
    ///
    /// E.g. `from_decimals` is used to get the token amount for BTC from the sats amount.
    pub fn from_decimals(amount: u128, decimals: u8) -> Self {
        Self { amount, decimals }
    }

    /// Create a new TokenAmount from the given integer amount and decimals.
    ///
    /// E.g. `from_int` is used to get the token amount for BTC from the integer amount.
    pub fn from_int(amount: u128, decimals: u8) -> Self {
        let amount = amount * 10u128.pow(decimals as u32);
        Self { amount, decimals }
    }

    /// Get the absolute amount of the token.
    pub fn amount(&self) -> u128 {
        self.amount
    }

    /// Get the integer amount of the token.
    pub fn as_int(&self) -> u128 {
        self.amount / 10u128.pow(self.decimals as u32)
    }
}

impl From<Decimal> for TokenAmount {
    fn from(value: Decimal) -> Self {
        use rust_decimal::prelude::ToPrimitive;
        let decimals = value.scale() as u8;
        Self::from_int(value.to_u128().unwrap(), decimals)
    }
}

impl From<TokenAmount> for Decimal {
    fn from(value: TokenAmount) -> Self {
        Decimal::new(value.amount() as i64, value.decimals as u32)
    }
}

impl Display for TokenAmount {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.amount())
    }
}

impl Add for TokenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if self.decimals != rhs.decimals {
            panic!("Cannot add TokenAmounts with different decimals")
        }

        Self::from_decimals(self.amount + rhs.amount, self.decimals)
    }
}

impl Sub for TokenAmount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.decimals != rhs.decimals {
            panic!("Cannot sub TokenAmounts with different decimals")
        }

        Self::from_decimals(self.amount - rhs.amount, self.decimals)
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_convert_from_numbers() {
        const HUNDRED_BTC: u128 = 10_000_000_000;
        assert_eq!(
            TokenAmount::from_decimals(HUNDRED_BTC, 8),
            TokenAmount::from_int(100, 8)
        );

        assert_eq!(TokenAmount::from_int(100, 8).amount(), HUNDRED_BTC);

        assert_eq!(TokenAmount::from_int(100, 8).as_int(), 100);
    }
}
