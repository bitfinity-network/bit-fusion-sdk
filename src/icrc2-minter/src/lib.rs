use bridge_canister::BridgeCanister;
use ic_metrics::Metrics;
pub use state::SigningStrategy;

pub use crate::canister::MinterCanister;

pub mod canister;
mod constant;
pub mod ops;
pub mod state;
pub mod tokens;

pub fn idl() -> String {
    let minter_canister_idl = MinterCanister::idl();

    let mut metrics_idl = <MinterCanister as Metrics>::get_idl();
    let mut bridge_idl = <MinterCanister as BridgeCanister>::get_idl();

    metrics_idl.merge(&minter_canister_idl);
    bridge_idl.merge(&metrics_idl);

    candid::pretty::candid::compile(&bridge_idl.env.env, &Some(bridge_idl.actor))
}
