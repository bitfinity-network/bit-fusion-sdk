pub mod canister;
pub mod ckbtc_client;
pub mod interface;
pub mod memory;
pub mod ops;
pub mod state;

pub use crate::canister::BtcBridge;

const MAINNET_CHAIN_ID: u32 = 0;
const TESTNET_CHAIN_ID: u32 = 1;
const REGTEST_CHAIN_ID: u32 = 2;

#[cfg(target_family = "wasm")]
#[ic_canister::export_candid]
pub fn idl() -> String {
    use bridge_canister::BridgeCanister;
    use ic_metrics::Metrics;

    let btc_bridge_canister_idl = BtcBridge::idl();

    let mut metrics_idl = <BtcBridge as Metrics>::get_idl();
    let mut bridge_idl = <BtcBridge as BridgeCanister>::get_idl();

    metrics_idl.merge(&btc_bridge_canister_idl);
    bridge_idl.merge(&metrics_idl);

    candid::pretty::candid::compile(&bridge_idl.env.env, &Some(bridge_idl.actor))
}
