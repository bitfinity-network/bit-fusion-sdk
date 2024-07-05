use alloy_sol_types::sol;
pub mod bft_events;
pub mod evm_bridge;
pub mod evm_link;
pub mod mint_orders;
pub mod operation_store;
pub mod query;

sol! {
    #[derive(Debug)]
    BFTBridge,
    "../../solidity/out/BftBridge.sol/BFTBridge.json"
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
