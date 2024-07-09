//! Implementation of the common functions for BFT bridge canisters. The main entry point is
//! [`BridgeCanister`] trait that should be implemented by a canister to include common APIs and
//! functions.
//!
//! The crate also provides [`bridge_inspect`] function that provides common `inspect_message` logic
//! for common bridge APIs.
//!
//! [`build_data`] macro can be used to provide canister build data in the common format.
mod build_data;
mod canister;
mod config;
mod core;
mod inspect;
mod log_config;
mod memory;
mod signer;

pub use core::BridgeCore;

pub use canister::BridgeCanister;
pub use inspect::bridge_inspect;
