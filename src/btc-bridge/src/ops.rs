use crate::canister::{eth_address_to_subaccount, get_scheduler};
use crate::ck_btc_interface::{UpdateBalanceArgs, UpdateBalanceError, UtxoStatus};
use crate::interface::{Erc20MintError, Erc20MintStatus};
use crate::scheduler::BtcTask;
use crate::state::State;
use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::utils::keccak256;
use ic_canister::virtual_canister_call;
use ic_exports::ic_kit::ic;
use ic_stable_structures::CellStructure;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::TaskOptions;
use minter_did::id256::Id256;
use minter_did::order::{MintOrder, SignedMintOrder};
use std::cell::RefCell;
use std::rc::Rc;

pub async fn btc_to_erc20(
    state: Rc<RefCell<State>>,
    eth_address: H160,
) -> Vec<Result<Erc20MintStatus, Erc20MintError>> {
    match request_update_balance(&state, &eth_address).await {
        Ok(minted_utxos) => {
            let mut results = vec![];
            for utxo in minted_utxos {
                let eth_address = eth_address.clone();
                let res = match utxo {
                    UtxoStatus::Minted {
                        minted_amount,
                        utxo,
                        ..
                    } => mint_erc20(&state, eth_address, minted_amount, utxo.height).await,
                    UtxoStatus::ValueTooSmall(utxo) => Err(Erc20MintError::ValueTooSmall(utxo)),
                    UtxoStatus::Tainted(utxo) => Err(Erc20MintError::Tainted(utxo)),
                    UtxoStatus::Checked(_) => Err(Erc20MintError::CkBtcError(
                        UpdateBalanceError::TemporarilyUnavailable(
                            "KYT check passed, but mint failed. Try again later.".to_string(),
                        ),
                    )),
                };

                results.push(res);
            }

            results
        }
        Err(UpdateBalanceError::NoNewUtxos {
            current_confirmations: None,
            ..
        }) => vec![Err(Erc20MintError::NothingToMint)],
        Err(UpdateBalanceError::NoNewUtxos {
            current_confirmations: Some(curr_confirmations),
            required_confirmations,
            pending_utxos,
        }) => {
            schedule_mint(eth_address);
            vec![Ok(Erc20MintStatus::Scheduled {
                current_confirmations: curr_confirmations,
                required_confirmations,
                pending_utxos,
            })]
        }
        Err(err) => vec![Err(Erc20MintError::CkBtcError(err))],
    }
}

async fn request_update_balance(
    state: &RefCell<State>,
    eth_address: &H160,
) -> Result<Vec<UtxoStatus>, UpdateBalanceError> {
    let self_id = ic::id();
    let ck_btc_minter = state.borrow().ck_btc_minter();
    let subaccount = eth_address_to_subaccount(eth_address);

    let args = UpdateBalanceArgs {
        owner: Some(self_id),
        subaccount: Some(subaccount),
    };

    virtual_canister_call!(
        ck_btc_minter,
        "update_balance",
        (args,),
        Result<Vec<UtxoStatus>, UpdateBalanceError>
    )
    .await
    .unwrap_or_else(|err| {
        Err(UpdateBalanceError::TemporarilyUnavailable(format!(
            "Failed to connect to ckBTC minter: {err:?}"
        )))
    })
}

fn schedule_mint(eth_address: H160) {
    let scheduler = get_scheduler();
    let scheduler = scheduler.borrow_mut();
    let task = BtcTask::MintErc20(eth_address);
    let options = TaskOptions::new();
    scheduler.append_task(task.into_scheduled(options));
}

pub async fn mint_erc20(
    state: &RefCell<State>,
    eth_address: H160,
    amount: u64,
    nonce: u32,
) -> Result<Erc20MintStatus, Erc20MintError> {
    let mint_order = prepare_mint_order(state, eth_address, amount, nonce).await?;
    Ok(match send_mint_order(state, mint_order).await {
        Ok(tx_id) => Erc20MintStatus::Minted { amount, tx_id },
        Err(err) => {
            log::warn!("Failed to send mint order: {err:?}");
            Erc20MintStatus::Signed(Box::new(mint_order))
        }
    })
}

async fn prepare_mint_order(
    state: &RefCell<State>,
    eth_address: H160,
    amount: u64,
    nonce: u32,
) -> Result<SignedMintOrder, Erc20MintError> {
    log::trace!("preparing mint order");

    let (sender, signer, mint_order) = {
        let state_ref = state.borrow();

        let sender_chain_id = state_ref.btc_chain_id();
        let sender = Id256::from_evm_address(&eth_address, sender_chain_id);
        let src_token = (&state_ref.ck_btc_ledger()).into();

        let recipient_chain_id = state_ref.erc20_chain_id();

        let mint_order = MintOrder {
            amount: amount.into(),
            sender,
            src_token,
            recipient: eth_address.clone(),
            dst_token: H160::default(),
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name: state_ref.token_name(),
            symbol: state_ref.token_symbol(),
            decimals: state_ref.decimals(),
        };

        let signer = state_ref.signer().get().clone();

        (sender, signer, mint_order)
    };

    let signed_mint_order = mint_order
        .encode_and_sign(&signer)
        .await
        .map_err(|err| Erc20MintError::Sign(format!("{err:?}")))?;

    let res = MintOrder::decode_signed(&signed_mint_order).unwrap().1;
    let hash = keccak256(&signed_mint_order.0[0..MintOrder::ENCODED_DATA_SIZE]);
    let res_ = ethers_core::types::Signature::from(res).recover(hash);
    ic::print(format!("{res_:?}"));

    state
        .borrow_mut()
        .mint_orders_mut()
        .push(sender, nonce, signed_mint_order);

    log::trace!("Mint order added");

    Ok(signed_mint_order)
}

async fn send_mint_order(
    state: &RefCell<State>,
    mint_order: SignedMintOrder,
) -> Result<H256, Erc20MintError> {
    log::trace!("Sending mint transaction");

    let signer = state.borrow().signer().get().clone();
    let sender = signer
        .get_address()
        .await
        .map_err(|err| Erc20MintError::Sign(format!("{err:?}")))?;

    let (evm_info, evm_params) = {
        let state = state.borrow();

        let evm_info = state.get_evm_info();
        let evm_params = state
            .get_evm_params()
            .clone()
            .ok_or(Erc20MintError::NotInitialized)?;

        (evm_info, evm_params)
    };

    let mut tx = minter_contract_utils::bft_bridge_api::mint_transaction(
        sender.0,
        evm_info.bridge_contract.0,
        evm_params.nonce.into(),
        evm_params.gas_price.into(),
        mint_order.to_vec(),
        evm_params.chain_id as _,
    );

    let signature = signer
        .sign_transaction(&(&tx).into())
        .await
        .map_err(|err| Erc20MintError::Sign(format!("{err:?}")))?;

    tx.r = signature.r.0;
    tx.s = signature.s.0;
    tx.v = signature.v.0;
    tx.hash = tx.hash();

    let client = evm_info.link.get_client();
    let id = client
        .send_raw_transaction(tx)
        .await
        .map_err(|err| Erc20MintError::Evm(format!("{err:?}")))?;

    log::trace!("Mint transaction sent");

    Ok(id.into())
}
