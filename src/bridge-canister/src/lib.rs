#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! Implementation of the common functions for BFT bridge canisters. The main entry point is
//! [`BridgeCanister`] trait that should be implemented by a canister to include common APIs and
//! functions.
//!
//! The crate also provides [`bridge_inspect`] function that provides common `inspect_message` logic
//! for common bridge APIs.
//!
//! [`build_data`] macro can be used to provide canister build data in the common format.

pub mod bridge;
mod build_data;
mod canister;
pub mod inspect;
mod log_config;
pub mod memory;
pub mod operation_store;
pub mod runtime;

pub use canister::BridgeCanister;
pub use inspect::bridge_inspect;
