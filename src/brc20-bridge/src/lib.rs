pub mod api;
pub mod canister;
pub mod constant;
pub mod memory;
pub mod ops;
pub mod scheduler;
pub mod state;
pub mod store;

use ic_metrics::Metrics;

pub use crate::canister::Brc20Bridge;

pub fn idl() -> String {
    let btc_bridge_idl = Brc20Bridge::idl();
    let mut metrics_idl = <Brc20Bridge as Metrics>::get_idl();
    metrics_idl.merge(&btc_bridge_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
