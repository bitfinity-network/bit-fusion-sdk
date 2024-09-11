pub mod canister;
pub mod constants;
pub mod core;
pub mod interface;
pub mod key;
pub mod ledger;
pub mod memory;
pub mod ops;
pub mod state;
pub mod task;

pub use crate::canister::RuneBridge;

const MAINNET_CHAIN_ID: u32 = 0;
const TESTNET_CHAIN_ID: u32 = 1;
const REGTEST_CHAIN_ID: u32 = 2;

#[cfg(target_family = "wasm")]
#[ic_canister::export_candid]
pub fn idl() -> String {
    use ic_metrics::Metrics;

    let btc_bridge_idl = RuneBridge::idl();
    let mut metrics_idl = <RuneBridge as Metrics>::get_idl();
    metrics_idl.merge(&btc_bridge_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
