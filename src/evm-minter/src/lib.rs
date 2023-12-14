mod canister;
mod memory;
mod state;

use ic_metrics::Metrics;

pub use crate::canister::SpenderCanister;

pub fn idl() -> String {
    let signature_verification_idl = SpenderCanister::idl();
    let mut metrics_idl = <SpenderCanister as Metrics>::get_idl();
    metrics_idl.merge(&signature_verification_idl);

    candid::bindings::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
