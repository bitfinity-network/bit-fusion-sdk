use serde::Deserialize;

/// Response for `/ordinals/v1/brc-20/tokens` endpoint.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GetBrc20TokensResponse {
    pub total: u64,
    pub results: Vec<Brc20TokenResponse>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Brc20TokenResponse {
    pub ticker: String,
    pub decimals: u8,
}

/// Response for `/ordinals/v1/brc-20/balances/{address}` endpoint.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GetBrc20BalancesResponse {
    pub total: u64,
    pub results: Vec<Brc20BalanceResponse>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Brc20BalanceResponse {
    pub ticker: String,
    pub overall_balance: u128,
}
