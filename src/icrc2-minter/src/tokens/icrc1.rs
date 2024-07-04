use std::cell::RefCell;
use std::collections::HashMap;

use candid::{CandidType, Nat, Principal};
use evm_canister_client::IcCanisterClient;
use icrc_client::account::Account;
use icrc_client::IcrcCanisterClient;
use minter_did::error::Result;
use num_traits::ToPrimitive as _;
use serde::{Deserialize, Serialize};

const ICRC1_METADATA_DECIMALS: &str = "icrc1:decimals";
const ICRC1_METADATA_NAME: &str = "icrc1:name";
const ICRC1_METADATA_SYMBOL: &str = "icrc1:symbol";

thread_local! {
    static TOKEN_CONFIGURATION: RefCell<HashMap<Principal, TokenConfiguration>> = RefCell::new(HashMap::default());
}

/// Get ICRC1 token configuration from cache if cached, otherwise fetch it and store it into the cache.
pub async fn get_token_configuration(ic_token: Principal) -> Result<TokenConfiguration> {
    if let Some(config) = TOKEN_CONFIGURATION
        .with(|token_configuration| token_configuration.borrow().get(&ic_token).cloned())
    {
        Ok(config)
    } else {
        let config = query_icrc1_configuration(ic_token).await?;
        cache_ic_token_configuration(config.clone());

        Ok(config)
    }
}

/// Get ICRC1 token configuration from cache.
pub fn get_cached_token_configuration(ic_token: Principal) -> Option<TokenConfiguration> {
    TOKEN_CONFIGURATION
        .with(|token_configuration| token_configuration.borrow().get(&ic_token).cloned())
}

/// Query token info from token canister and store it to cache.
/// Read the info from cache if query fails.
pub async fn query_token_info_or_read_from_cache(token: Principal) -> Option<TokenInfo> {
    let cached = get_cached_token_configuration(token);

    let Ok(queried) = query_icrc1_token_info(
        token,
        cached
            .as_ref()
            .map(|cached| cached.info.info_set_in_metadata)
            .unwrap_or(true), // if not in cache; always try to read from metadata first
    )
    .await
    else {
        return cached.map(|config| config.info);
    };

    // If we store token config in cache, update the config with new info.
    if let Some(mut config) = get_cached_token_configuration(token) {
        config.info = queried.clone();
        cache_ic_token_configuration(config);
    }

    Some(queried)
}

/// Get ICRC1 token configuration from token canister and store it to cache.
pub async fn refresh_token_configuration(ic_token: Principal) -> Result<TokenConfiguration> {
    let config = query_icrc1_configuration(ic_token).await?;
    cache_ic_token_configuration(config.clone());
    Ok(config)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, CandidType)]
pub struct TokenInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub info_set_in_metadata: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, CandidType)]
pub struct TokenConfiguration {
    pub principal: Principal,
    pub fee: Nat,
    pub minting_account: Account,
    pub info: TokenInfo,
}

/// Requests fee and minting account configuration from an ICRC-1 canister.
async fn query_icrc1_configuration(token: Principal) -> Result<TokenConfiguration> {
    let icrc_client = IcrcCanisterClient::new(IcCanisterClient::new(token));

    // ICRC-1 standard metadata doesn't include a minting account, so we have to do two requests
    // to get both fields, which is fine though since this is done once.
    let fee = icrc_client.icrc1_fee().await?;
    let minting_account = icrc_client
        .icrc1_minting_account()
        .await?
        .unwrap_or(Account {
            owner: Principal::management_canister(),
            subaccount: None,
        });

    let info = query_icrc1_token_info(token, true).await?;

    Ok(TokenConfiguration {
        principal: token,
        fee,
        minting_account,
        info,
    })
}

/// Requests fee and minting account configuration from an ICRC-1 canister.
///
/// `has_token_info_in_metadata` is a flag that indicates whether the token info is set in metadata.
/// If it is, we can use the standard ICRC-1 metadata query to get the token info.
/// Otherwise, we have to use dedicated queries to get the token info.
async fn query_icrc1_token_info(
    token: Principal,
    has_token_info_in_metadata: bool,
) -> Result<TokenInfo> {
    let icrc_client = IcrcCanisterClient::new(IcCanisterClient::new(token));

    // If the token info is set in metadata, we can use the standard ICRC-1 metadata query.
    if has_token_info_in_metadata {
        if let Ok(token_info) = query_icrc1_token_info_from_metadata(&icrc_client).await {
            return Ok(token_info);
        }
    }

    // Otherwise, we have to use dedicated queries to get the token info.
    query_icrc1_token_info_from_dedicated_queries(&icrc_client).await
}

/// Requests token info from an ICRC-1 canister using `icrc1_metadata` query.
async fn query_icrc1_token_info_from_metadata(
    client: &IcrcCanisterClient<IcCanisterClient>,
) -> Result<TokenInfo> {
    let token_metadata = client.icrc1_metadata().await?;
    let name = match get_metadata_value(&token_metadata, ICRC1_METADATA_NAME) {
        Some(icrc_client::Value::Text(name)) => name.clone(),
        _ => {
            return Err(minter_did::error::Error::Internal(
                "Bad icrc1 metadata".to_string(),
            ))
        }
    };
    let symbol = match get_metadata_value(&token_metadata, ICRC1_METADATA_SYMBOL) {
        Some(icrc_client::Value::Text(symbol)) => symbol.clone(),
        _ => {
            return Err(minter_did::error::Error::Internal(
                "Bad icrc1 metadata".to_string(),
            ))
        }
    };
    let decimals = match get_metadata_value(&token_metadata, ICRC1_METADATA_DECIMALS) {
        Some(icrc_client::Value::Nat(decimals)) => decimals.0.to_u8().ok_or(
            minter_did::error::Error::Internal("Bad icrc1 metadata".to_string()),
        ),
        _ => Err(minter_did::error::Error::Internal(
            "Bad icrc1 metadata".to_string(),
        )),
    }?;

    Ok(TokenInfo {
        name,
        symbol,
        decimals,
        info_set_in_metadata: true,
    })
}

/// Requests token info from an ICRC-1 canister using dedicated queries.
/// This is a fallback in case `icrc1_metadata` query doesn't return the standard ICRC-1 keys.
///
/// The fallback queries are: `icrc1_name`, `icrc1_symbol`, `icrc1_decimals`.
async fn query_icrc1_token_info_from_dedicated_queries(
    client: &IcrcCanisterClient<IcCanisterClient>,
) -> Result<TokenInfo> {
    let name = client.icrc1_name().await?;
    let symbol = client.icrc1_symbol().await?;
    let decimals = client.icrc1_decimals().await?;

    Ok(TokenInfo {
        name,
        symbol,
        decimals,
        info_set_in_metadata: false,
    })
}

/// Get the value of a metadata key from a list of metadata key-value pairs.
fn get_metadata_value<'a>(
    metadata: &'a [(String, icrc_client::Value)],
    key: &str,
) -> Option<&'a icrc_client::Value> {
    metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Cache the token configuration value in the cache
fn cache_ic_token_configuration(config: TokenConfiguration) {
    TOKEN_CONFIGURATION.with(|token_configuration| {
        token_configuration
            .borrow_mut()
            .insert(config.principal, config);
    });
}

#[cfg(test)]
mod test {
    use candid::Nat;
    use ic_exports::icrc_types::icrc1::account::Account;

    use super::*;

    #[tokio::test]
    async fn should_cache_config() {
        let ic_token = Principal::from_slice(&[42; 20]);

        let config = TOKEN_CONFIGURATION
            .with(|token_configuration| token_configuration.borrow().get(&ic_token).cloned());

        assert!(config.is_none());

        let config = TokenConfiguration {
            principal: ic_token,
            fee: Nat::from(24_u64),
            minting_account: Account {
                owner: Principal::from_slice(&[43; 20]),
                subaccount: None,
            },
            info: TokenInfo {
                name: "Test Token".to_string(),
                symbol: "TEST".to_string(),
                decimals: 18,
                info_set_in_metadata: true,
            },
        };

        cache_ic_token_configuration(config.clone());

        let cached_config = TOKEN_CONFIGURATION
            .with(|token_configuration| token_configuration.borrow().get(&ic_token).cloned())
            .unwrap();

        assert_eq!(config.principal, cached_config.principal);
        assert_eq!(config.fee, cached_config.fee);
        assert_eq!(config.minting_account, cached_config.minting_account);
        assert_eq!(config.info, cached_config.info);
    }
}
