use std::cell::RefCell;
use std::collections::HashMap;

use bridge_did::error::Result;
use candid::{CandidType, Nat, Principal};
use evm_canister_client::{CanisterClient, IcCanisterClient};
use icrc_client::account::Account;
use icrc_client::IcrcCanisterClient;
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
    let icrc_client = IcrcCanisterClient::new(IcCanisterClient::new(token));

    let Ok(queried) = query_icrc1_token_info(&icrc_client).await else {
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

    let info = query_icrc1_token_info(&icrc_client).await?;

    Ok(TokenConfiguration {
        principal: token,
        fee,
        minting_account,
        info,
    })
}

/// Requests token info from an ICRC-1 canister using `icrc1_metadata` query.
async fn query_icrc1_token_info<C>(client: &IcrcCanisterClient<C>) -> Result<TokenInfo>
where
    C: CanisterClient,
{
    let token_metadata = client.icrc1_metadata().await?;
    let name = match get_metadata_value(&token_metadata, ICRC1_METADATA_NAME) {
        Some(icrc_client::Value::Text(name)) => name.clone(),
        _ => {
            return Err(bridge_did::error::Error::Internal(
                "Bad icrc1 metadata".to_string(),
            ))
        }
    };
    let symbol = match get_metadata_value(&token_metadata, ICRC1_METADATA_SYMBOL) {
        Some(icrc_client::Value::Text(symbol)) => symbol.clone(),
        _ => {
            return Err(bridge_did::error::Error::Internal(
                "Bad icrc1 metadata".to_string(),
            ))
        }
    };
    let decimals = match get_metadata_value(&token_metadata, ICRC1_METADATA_DECIMALS) {
        Some(icrc_client::Value::Nat(decimals)) => decimals.0.to_u8().ok_or(
            bridge_did::error::Error::Internal("Bad icrc1 metadata".to_string()),
        ),
        _ => Err(bridge_did::error::Error::Internal(
            "Bad icrc1 metadata".to_string(),
        )),
    }?;

    Ok(TokenInfo {
        name,
        symbol,
        decimals,
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
    use evm_canister_client::{CanisterClient, CanisterClientResult};
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
        assert_eq!(config.info, cached_config.info);
    }

    #[tokio::test]
    async fn should_get_token_info() {
        let client = FakeIcrcCanisterClient {
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            decimals: 18,
        };
        let client = IcrcCanisterClient::new(client);

        // fetch with icrc1 metadata
        let token_info = query_icrc1_token_info(&client).await.unwrap();
        assert_eq!(token_info.name, "Test Token");
        assert_eq!(token_info.symbol, "TEST");
        assert_eq!(token_info.decimals, 18);
    }

    #[derive(Debug, Clone)]
    struct FakeIcrcCanisterClient {
        name: String,
        symbol: String,
        decimals: u8,
    }

    #[async_trait::async_trait]
    impl CanisterClient for FakeIcrcCanisterClient {
        async fn query<T, R>(&self, method: &str, _args: T) -> CanisterClientResult<R>
        where
            T: candid::utils::ArgumentEncoder + Send + Sync,
            R: serde::de::DeserializeOwned + CandidType,
        {
            let response: R = match method {
                "icrc1_metadata" => {
                    let metadata = vec![
                        (
                            ICRC1_METADATA_NAME.to_string(),
                            icrc_client::Value::Text(self.name.clone()),
                        ),
                        (
                            ICRC1_METADATA_SYMBOL.to_string(),
                            icrc_client::Value::Text(self.symbol.clone()),
                        ),
                        (
                            ICRC1_METADATA_DECIMALS.to_string(),
                            icrc_client::Value::Nat(Nat((self.decimals as u64).into())),
                        ),
                    ];

                    let json = serde_json::to_value(metadata).unwrap();

                    serde_json::from_value::<R>(json).unwrap()
                }
                "icrc1_name" => {
                    let json = serde_json::to_value(self.name.clone()).unwrap();
                    serde_json::from_value::<R>(json).unwrap()
                }
                "icrc1_symbol" => {
                    let json = serde_json::to_value(self.symbol.clone()).unwrap();
                    serde_json::from_value::<R>(json).unwrap()
                }
                "icrc1_decimals" => {
                    let json = serde_json::to_value(self.decimals).unwrap();
                    serde_json::from_value::<R>(json).unwrap()
                }
                _ => panic!("Unexpected method: {}", method),
            };

            Ok(response)
        }

        async fn update<T, R>(&self, _method: &str, _args: T) -> CanisterClientResult<R>
        where
            T: candid::utils::ArgumentEncoder + Send + Sync,
            R: serde::de::DeserializeOwned + CandidType,
        {
            unimplemented!()
        }
    }
}
