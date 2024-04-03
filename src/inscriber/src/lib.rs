mod accessor;
#[allow(unused)]
mod build_data;
pub mod canister;
mod constant;
pub mod http;
pub mod ops;
pub mod wallet;

use std::cell::RefCell;
use std::time::Duration;

use candid::Principal;
use ic_metrics::Metrics;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

pub use crate::canister::Inscriber;

thread_local! {
    pub static RNG: RefCell<Option<StdRng>> = const { RefCell::new(None) };
}

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
pub fn register_custom_getrandom() {
    ic_exports::ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_exports::ic_cdk::spawn(set_rand())
    });
    getrandom::register_custom_getrandom!(custom_rand);
}

fn custom_rand(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    RNG.with(|rng| rng.borrow_mut().as_mut().unwrap().fill_bytes(buf));
    Ok(())
}

async fn set_rand() {
    let (seed,) = ic_exports::ic_cdk::call(Principal::management_canister(), "raw_rand", ())
        .await
        .unwrap();
    RNG.with(|rng| {
        *rng.borrow_mut() = Some(StdRng::from_seed(seed));
        log::debug!("rng: {:?}", *rng.borrow());
    });
}
