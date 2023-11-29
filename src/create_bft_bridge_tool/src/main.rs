use candid::{CandidType, IDLArgs, TypeEnv};
use clap::Parser;
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::Transaction;
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::{Signer, Wallet};
use ethereum_types::H160;
use ethers_core::abi::Token;
use minter_contract_utils::bft_bridge_api;
use minter_contract_utils::build_data::BFT_BRIDGE_SMART_CONTRACT_CODE;

/// Cli args
#[derive(Parser, Debug)]
#[clap(
    version = "0.1",
    about = "Tool for outputting a transaction call to create a BftBridge contract in EVM canister"
)]
struct CliArgs {
    /// Address of minter canister
    #[arg(long)]
    minter_address: String,

    /// EVM canister chain id
    #[arg(long)]
    chain_id: u32,
}

fn main() {
    let args = CliArgs::parse();
    let minter = H160::from_slice(
        &hex::decode(args.minter_address.trim_start_matches("0x"))
            .expect("failed to parse minter address"),
    );
    let chain_id = args.chain_id;

    let sender = Wallet::new(&mut rand::thread_rng());

    let input = bft_bridge_api::CONSTRUCTOR
        .encode_input(
            BFT_BRIDGE_SMART_CONTRACT_CODE.clone(),
            &[Token::Address(minter)],
        )
        .unwrap();

    let create_contract_tx = TransactionBuilder {
        from: &sender.address().into(),
        to: None,
        nonce: 0u64.into(),
        value: 0u64.into(),
        gas: 3500000u64.into(),
        gas_price: Some(EIP1559_INITIAL_BASE_FEE.into()),
        input,
        signature: SigningMethod::SigningKey(sender.signer()),
        chain_id: chain_id as _,
    }
    .calculate_hash_and_build()
    .expect("failed to sign the transaction");

    let candid_bytes =
        candid::encode_args((&create_contract_tx,)).expect("failed to serialize tx to Candid");
    let args = IDLArgs::from_bytes(&candid_bytes).expect("failed to deserialize Candid");
    // Without type annotation instead of field names numerical ids will be used in output
    let args = args
        .annotate_types(false, &TypeEnv::new(), &[Transaction::ty()])
        .unwrap();

    // Output the transaction in Candid text format
    println!("{args}");
}
