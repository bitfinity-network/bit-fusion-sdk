use super::state::*;
use crate::http::{http_get_req, PaginatedResp};

/// Retrieves information for `BRC-20` tokens.
pub async fn get_brc20_tokens(
    base_api_url: &str,
    offset: u64,
    limit: u64,
) -> Result<Option<PaginatedResp<Brc20Token>>, String> {
    http_get_req::<PaginatedResp<Brc20Token>>(&format!(
        "{base_api_url}/ordinals/v1/brc-20/tokens?offset={offset}&limit={limit}"
    ))
    .await
}

/// Retrieves information for a `BRC-20` token including supply and holders.
pub async fn get_brc20_token_by_ticker(
    base_api_url: &str,
    ticker: &str,
) -> Result<Option<Brc20TokenDetails>, String> {
    http_get_req::<Brc20TokenDetails>(&format!(
        "{base_api_url}/ordinals/v1/brc-20/tokens/{ticker}"
    ))
    .await
}

/// Retrieves a list of holders and their balances for a `BRC-20` token.
pub async fn get_brc20_token_holders_by_ticker(
    base_api_url: &str,
    ticker: &str,
    offset: u64,
    limit: u64,
) -> Result<Option<PaginatedResp<Brc20Holder>>, String> {
    http_get_req::<PaginatedResp<Brc20Holder>>(&format!(
        "{base_api_url}/ordinals/v1/brc-20/tokens/{ticker}/holders?offset={offset}&limit={limit}"
    ))
    .await
}

/// Retrieves `BRC-20` token balances for a Bitcoin address.
pub async fn get_brc20_token_balance_by_address(
    base_api_url: &str,
    address: &str,
    ticker: &str,
) -> Result<Option<PaginatedResp<Brc20Balance>>, String> {
    http_get_req::<PaginatedResp<Brc20Balance>>(&format!(
        "{base_api_url}/ordinals/v1/brc-20/balances/{address}?ticker={ticker}"
    ))
    .await
}
