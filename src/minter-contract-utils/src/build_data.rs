use once_cell::sync::Lazy;

/// Wrapped token contract bytecode
const BUILD_SMART_CONTRACT_WRAPPED_TOKEN_HEX_CODE: &str =
    env!("BUILD_SMART_CONTRACT_WRAPPED_TOKEN_HEX_CODE");

/// Bridge contract bytecode
const BUILD_SMART_CONTRACT_BFT_BRIDGE_HEX_CODE: &str =
    env!("BUILD_SMART_CONTRACT_BFT_BRIDGE_HEX_CODE");

/// Bridge contract deployed bytecode
const BUILD_SMART_CONTRACT_BFT_BRIDGE_DEPLOYED_HEX_CODE: &str =
    env!("BUILD_SMART_CONTRACT_BFT_BRIDGE_DEPLOYED_HEX_CODE");

/// Get contract code from the environment variable
fn get_contract_code(env_name: &str) -> Vec<u8> {
    hex::decode(env_name)
        .unwrap_or_else(|_| panic!("failed to decode smart contract bytecode from '{env_name}'"))
}

/// WrappedToken smart contract bytecode
pub static WRAPPED_TOKEN_SMART_CONTRACT_CODE: Lazy<Vec<u8>> =
    Lazy::new(|| get_contract_code(BUILD_SMART_CONTRACT_WRAPPED_TOKEN_HEX_CODE));

/// BftBridge smart contract bytecode
pub static BFT_BRIDGE_SMART_CONTRACT_CODE: Lazy<Vec<u8>> =
    Lazy::new(|| get_contract_code(BUILD_SMART_CONTRACT_BFT_BRIDGE_HEX_CODE));

/// BftBridge smart contract deployed bytecode
pub static BFT_BRIDGE_SMART_CONTRACT_DEPLOYED_CODE: Lazy<Vec<u8>> =
    Lazy::new(|| get_contract_code(BUILD_SMART_CONTRACT_BFT_BRIDGE_DEPLOYED_HEX_CODE));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_get_wrapped_token_smart_contract_code() {
        let code = &*WRAPPED_TOKEN_SMART_CONTRACT_CODE;
        assert!(!code.is_empty())
    }

    #[test]
    fn should_get_bft_bridge_token_smart_contract_code() {
        let code = &*BFT_BRIDGE_SMART_CONTRACT_CODE;
        assert!(!code.is_empty())
    }

    #[test]
    fn should_get_bft_bridge_token_smart_contract_deployed_code() {
        let code = &*BFT_BRIDGE_SMART_CONTRACT_DEPLOYED_CODE;
        assert!(!code.is_empty())
    }
}
