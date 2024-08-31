use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bridge_canister::memory::{memory_by_id, StableMemory};
use bridge_canister::operation_store::OperationsMemory;
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::{SharedConfig, State};
use bridge_did::id256::Id256;
use ic_stable_structures::MemoryId;

use super::*;
use crate::ops::tests::sign_mint_order::TestSigner;

mod await_confirmations;
mod await_inputs;
mod deposit_request;
mod send_mint_order;
mod sign_mint_order;

fn op_memory() -> OperationsMemory<StableMemory> {
    OperationsMemory {
        id_counter: memory_by_id(MemoryId::new(1)),
        incomplete_operations: memory_by_id(MemoryId::new(2)),
        operations_log: memory_by_id(MemoryId::new(3)),
        operations_map: memory_by_id(MemoryId::new(4)),
        memo_operations_map: memory_by_id(MemoryId::new(5)),
    }
}

fn config() -> SharedConfig {
    Rc::new(RefCell::new(ConfigStorage::default(memory_by_id(
        MemoryId::new(5),
    ))))
}

fn test_state() -> RuntimeState<RuneBridgeOp> {
    Rc::new(RefCell::new(State::default(op_memory(), config())))
}

fn sender() -> H160 {
    H160::from_slice(&[1; 20])
}

fn rune_name(name: &str) -> RuneName {
    RuneName::from_str(name).unwrap()
}

fn token_address(v: u8) -> H160 {
    H160::from_slice(&[v; 20])
}

fn dst_tokens() -> HashMap<RuneName, H160> {
    [
        (rune_name("AAA"), token_address(2)),
        (rune_name("A"), token_address(3)),
        (rune_name("B"), token_address(4)),
    ]
    .into()
}

fn test_mint_order() -> MintOrder {
    MintOrder {
        amount: Default::default(),
        sender: Id256::from_evm_address(&H160::from_slice(&[1; 20]), 1),
        src_token: Id256::from_evm_address(&H160::from_slice(&[2; 20]), 1),
        recipient: Default::default(),
        dst_token: Default::default(),
        nonce: 0,
        sender_chain_id: 0,
        recipient_chain_id: 0,
        name: [1; 32],
        symbol: [1; 16],
        decimals: 0,
        approve_spender: Default::default(),
        approve_amount: Default::default(),
        fee_payer: Default::default(),
    }
}

async fn test_signed_order() -> SignedMintOrder {
    test_mint_order()
        .encode_and_sign(&TestSigner::ok())
        .await
        .unwrap()
}
