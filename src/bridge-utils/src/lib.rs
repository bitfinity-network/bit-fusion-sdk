use alloy_sol_types::sol;
pub mod btf_events;
pub mod common;
pub mod evm_bridge;
pub mod evm_link;
pub mod query;

sol! {
    #[derive(Debug)]
    BTFBridge,
    "../../solidity/out/BTFbridge.sol/BTFBridge.json"
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
