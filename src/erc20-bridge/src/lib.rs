pub mod canister;
pub mod memory;
pub mod ops;
pub mod state;

pub use crate::canister::Erc20Bridge;

#[cfg(target_family = "wasm")]
#[export_candid]
pub fn idl() -> String {

    use ic_canister::export_candid;
    use ic_metrics::Metrics;
    
    let signature_verification_idl = Erc20Bridge::idl();
    let mut metrics_idl = <Erc20Bridge as Metrics>::get_idl();
    metrics_idl.merge(&signature_verification_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
