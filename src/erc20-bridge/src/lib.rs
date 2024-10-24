pub mod canister;
pub mod memory;
pub mod ops;
pub mod state;

pub use crate::canister::Erc20Bridge;

#[cfg(target_family = "wasm")]
#[ic_canister::export_candid]
pub fn idl() -> String {
    use bridge_canister::BridgeCanister;
    use ic_metrics::Metrics;

    let bridge_canister_idl = Erc20Bridge::idl();

    let mut metrics_idl = <Erc20Bridge as Metrics>::get_idl();
    let mut bridge_idl = <Erc20Bridge as BridgeCanister>::get_idl();

    metrics_idl.merge(&bridge_canister_idl);
    bridge_idl.merge(&metrics_idl);

    candid::pretty::candid::compile(&bridge_idl.env.env, &Some(bridge_idl.actor))
}
