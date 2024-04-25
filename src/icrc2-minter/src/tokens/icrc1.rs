use std::cell::RefCell;
use std::collections::HashMap;

use candid::{CandidType, Nat, Principal};
use evm_canister_client::IcCanisterClient;
use ic_canister::virtual_canister_call;
use ic_exports::icrc_types::icrc1::account::Account;
use icrc_client::IcrcCanisterClient;
use minter_did::error::Result;
use serde::{Deserialize, Serialize};

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
    let Ok(queried) = query_icrc1_token_info(token).await else {
        return get_cached_token_configuration(token).map(|config| config.info);
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

    let name = icrc_client.icrc1_name().await?;
    let symbol = icrc_client.icrc1_symbol().await?;
    let decimals = icrc_client.icrc1_decimals().await?;

    let info = TokenInfo {
        name,
        symbol,
        decimals,
    };

    Ok(TokenConfiguration {
        principal: token,
        fee,
        minting_account,
        info,
    })
}

/// Requests fee and minting account configuration from an ICRC-1 canister.
async fn query_icrc1_token_info(token: Principal) -> Result<TokenInfo> {
    let icrc_client = IcrcCanisterClient::new(IcCanisterClient::new(token));

    let name = icrc_client.icrc1_name().await?;
    let symbol = icrc_client.icrc1_symbol().await?;
    let decimals = icrc_client.icrc1_decimals().await?;

    Ok(TokenInfo {
        name,
        symbol,
        decimals,
    })
}

/// Cache the token configuration value in the cache
fn cache_ic_token_configuration(config: TokenConfiguration) {
    TOKEN_CONFIGURATION.with(|token_configuration| {
        token_configuration
            .borrow_mut()
            .insert(config.principal, config);
    });
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize, Serialize, CandidType)]
pub struct IcrcTransferDst {
    pub token: Principal,
    pub recipient: Principal,
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
            },
        };

        cache_ic_token_configuration(config.clone());

        let cached_config = TOKEN_CONFIGURATION
            .with(|token_configuration| token_configuration.borrow().get(&ic_token).cloned())
            .unwrap();

        assert_eq!(config.principal, cached_config.principal);
        assert_eq!(config.fee, cached_config.fee);
        assert_eq!(config.minting_account, cached_config.minting_account);
    }
}
