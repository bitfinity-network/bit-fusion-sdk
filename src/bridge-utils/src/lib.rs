use alloy_sol_types::sol;
mod address;
pub mod btf_events;
pub mod common;
pub mod evm_bridge;
pub mod evm_link;
pub mod query;

pub use self::address::get_contract_address;

#[cfg(feature = "native")]
pub mod native;

sol! {
    #[derive(Debug)]
    BTFBridge,
    "../../solidity/out/BTFBridge.sol/BTFBridge.json"
}

sol! {
    #[derive(Debug)]
    UUPSProxy,
    "../../solidity/out/UUPSProxy.sol/UUPSProxy.json"
}

sol! {
    #[derive(Debug)]
    FeeCharge,
    "../../solidity/out/FeeCharge.sol/FeeCharge.json"
}

sol! {
    #[derive(Debug)]
    WrappedToken,
    "../../solidity/out/WrappedToken.sol/WrappedToken.json"
}

sol! {
    #[derive(Debug)]
    WrappedTokenDeployer,
    "../../solidity/out/WrappedTokenDeployer.sol/WrappedTokenDeployer.json"
}
