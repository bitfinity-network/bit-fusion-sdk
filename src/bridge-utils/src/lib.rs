use alloy_sol_types::sol;
pub mod bft_events;
pub mod evm_bridge;
pub mod evm_link;
pub mod mint_orders;
pub mod operation_store;
pub mod query;

use serde::{Deserialize, Serialize};

sol! {
    #[sol(abi=true)]
    #[derive(Debug, Serialize, Deserialize)]
    BFTBridge,
    "../../solidity/out/BftBridge.sol/BFTBridge.json"
}

sol! {
    #[sol(abi=true)]
    #[derive(Debug)]
    UUPSProxy,
    "../../solidity/out/UUPSProxy.sol/UUPSProxy.json"
}

sol! {
    #[sol(abi=true)]
    #[derive(Debug)]
    FeeCharge,
    "../../solidity/out/FeeCharge.sol/FeeCharge.json"
}

sol! {
    #[sol(abi=true)]
    #[derive(Debug)]
    WrappedToken,
    "../../solidity/out/WrappedToken.sol/WrappedToken.json"
}
