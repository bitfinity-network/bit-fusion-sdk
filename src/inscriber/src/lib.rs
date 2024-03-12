#[allow(unused)]
mod build_data;
mod constants;

pub mod canister;
pub mod wallet;

use ic_metrics::Metrics;

pub use crate::canister::Inscriber;

pub fn idl() -> String {
    let inscriber_idl = Inscriber::idl();
    let mut metrics_idl = <Inscriber as Metrics>::get_idl();
    metrics_idl.merge(&inscriber_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}

// In the following, we register a custom `getrandom` implementation because
// otherwise `getrandom` (which is an indirect dependency of `bitcoin`) fails to compile.
// This is necessary because `getrandom` by default fails to compile for the
// `wasm32-unknown-unknown` target (which is required for deploying a canister).
getrandom::register_custom_getrandom!(always_fail);
pub fn always_fail(_buf: &mut [u8]) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}
