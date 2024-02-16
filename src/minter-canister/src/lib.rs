mod build_data;
mod canister;
mod constant;
mod context;
mod evm;
mod memory;
pub mod state;
pub mod tokens;

use std::marker::PhantomData;
use std::rc::Rc;

use ic_metrics::Metrics;
pub use state::SigningStrategy;

pub use crate::canister::MinterCanister;

type ForceNotSendAndNotSync = PhantomData<Rc<()>>;

pub fn idl() -> String {
    let minter_canister_idl = MinterCanister::idl();

    let mut metrics_idl = <MinterCanister as Metrics>::get_idl();
    metrics_idl.merge(&minter_canister_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
