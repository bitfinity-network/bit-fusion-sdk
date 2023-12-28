pub mod canister;
pub mod client;
pub mod memory;
pub mod state;
pub mod tasks;

use ic_metrics::Metrics;

pub use crate::canister::EvmMinter;

pub fn idl() -> String {
    let signature_verification_idl = EvmMinter::idl();
    let mut metrics_idl = <EvmMinter as Metrics>::get_idl();
    metrics_idl.merge(&signature_verification_idl);

    candid::bindings::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
