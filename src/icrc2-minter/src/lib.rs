use std::marker::PhantomData;
use std::rc::Rc;

use ic_metrics::Metrics;
pub use state::SigningStrategy;

pub use crate::canister::MinterCanister;

mod build_data;
pub mod canister;
mod constant;
mod memory;
pub mod operation;
pub mod state;
mod tasks;
pub mod tokens;

type ForceNotSendAndNotSync = PhantomData<Rc<()>>;

pub fn idl() -> String {
    let minter_canister_idl = MinterCanister::idl();

    let mut metrics_idl = <MinterCanister as Metrics>::get_idl();
    metrics_idl.merge(&minter_canister_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
