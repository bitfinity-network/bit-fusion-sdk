pub mod canister;
pub mod ck_btc_interface;
pub mod interface;
pub mod memory;
pub mod ops;
pub mod orders_store;
pub mod scheduler;
pub mod state;

use ic_metrics::Metrics;

pub use crate::canister::BtcBridge;

pub fn idl() -> String {
    let btc_bridge_idl = BtcBridge::idl();
    let mut metrics_idl = <BtcBridge as Metrics>::get_idl();
    metrics_idl.merge(&signature_verification_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
