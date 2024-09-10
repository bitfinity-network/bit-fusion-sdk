pub use state::SigningStrategy;

pub use crate::canister::Icrc2BridgeCanister;

pub mod canister;
mod constant;
pub mod ops;
pub mod state;
pub mod tokens;

#[cfg(target_family = "wasm")]
#[ic_canister::export_candid]
fn idl() -> String {
    use bridge_canister::BridgeCanister;
    use ic_metrics::Metrics;

    let bridge_canister_idl = Icrc2BridgeCanister::idl();

    let mut metrics_idl = <Icrc2BridgeCanister as Metrics>::get_idl();
    let mut bridge_idl = <Icrc2BridgeCanister as BridgeCanister>::get_idl();

    metrics_idl.merge(&bridge_canister_idl);
    bridge_idl.merge(&metrics_idl);

    candid::pretty::candid::compile(&bridge_idl.env.env, &Some(bridge_idl.actor))
}
