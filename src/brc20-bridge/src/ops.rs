use core::sync::atomic::Ordering;
use std::cell::RefCell;

use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_stable_structures::CellStructure;
use inscriber::ecdsa_api::IcBtcSigner;
use inscriber::interface::{InscribeTransactions, Protocol};
use inscriber::ops as Inscriber;
use minter_did::id256::Id256;
use minter_did::order::{MintOrder, SignedMintOrder};
use ord_rs::{Inscription as _, InscriptionId};

use crate::constant::NONCE;
use crate::interface::bridge_api::{
    Brc20InscribeStatus, DepositBrc20Args, DepositError, Erc20MintStatus, WithdrawError,
};
use crate::rpc;
use crate::state::State;

/// Swaps a BRC20 for an ERC20.
///
/// This burns a BRC20 and mints an equivalent ERC20.
pub async fn brc20_to_erc20(
    state: &RefCell<State>,
    eth_address: H160,
    brc20: DepositBrc20Args,
) -> Result<Vec<Erc20MintStatus>, DepositError> {
    let DepositBrc20Args { tx_id, ticker: _ } = brc20;

    // TODO: https://infinityswap.atlassian.net/browse/EPROD-858
    //
    // log::info!("Fetching BRC20 token details");
    // let _fetched_token = rpc::fetch_brc20_token_details(state, &ticker)
    //     .await?;

    log::info!("Fetching BRC20 transfer transaction by its ID: {tx_id}");
    let transaction = rpc::fetch_transfer_transaction(state, &eth_address, &tx_id).await?;

    log::info!("Parsing BRC20 inscriptions from from the given transaction");
    let storable_brc20s = rpc::parse_and_validate_inscriptions(state, transaction).await?;

    if storable_brc20s.is_empty() {
        return Err(DepositError::NothingToDeposit);
    }
    log::debug!("Parsed BRC20 inscriptions: {:?}", storable_brc20s);

    state
        .borrow_mut()
        .inscriptions_mut()
        .write_all(&storable_brc20s);

    let mintable_brc20s = storable_brc20s
        .iter()
        .map(|brc20| rpc::get_brc20_data(brc20.clone().actual_brc20()))
        .collect::<Vec<(u64, String)>>();

    let mut mint_results = vec![];

    for (amount, tick) in mintable_brc20s {
        // Set the token symbol using the tick (symbol) from the BRC20
        state.borrow_mut().set_token_symbol(&tick)?;

        let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
        log::info!("Minting an ERC20 token with symbol: {tick}");

        match mint_erc20(state, &eth_address, amount, nonce).await {
            Ok(status) => mint_results.push(status),
            Err(err) => {
                log::error!("Failed to mint ERC20 token for {tick}: {err:?}");
                return Err(err);
            }
        };
    }
    Ok(mint_results)
}

pub async fn mint_erc20(
    state: &RefCell<State>,
    eth_address: &H160,
    amount: u64,
    nonce: u32,
) -> Result<Erc20MintStatus, DepositError> {
    // let fee = state.borrow().deposit_fee();
    // let amount_minus_fee = amount
    //     .checked_sub(fee)
    //     .ok_or(DepositError::ValueTooSmall(amount.to_string()))?;

    let mint_order = prepare_mint_order(state, eth_address.clone(), amount, nonce).await?;
    store_mint_order(state, mint_order, eth_address, nonce);

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
) -> Result<SignedMintOrder, DepositError> {
    log::info!("preparing mint order");

    let (signer, mint_order) = {
        let state_ref = state.borrow();

        let sender_chain_id = state_ref.btc_chain_id();
        let sender = Id256::from_evm_address(&eth_address, sender_chain_id);
        let src_token = Id256::from(&ic_exports::ic_kit::ic::id());

        let recipient_chain_id = state_ref.erc20_chain_id();

        let mint_order = MintOrder {
            amount: amount.into(),
            sender,
            src_token,
            recipient: eth_address,
            dst_token: H160::default(),
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name: state_ref.token_name(),
            symbol: state_ref.token_symbol(),
            decimals: state_ref.decimals(),
            approve_spender: Default::default(),
            approve_amount: Default::default(),
            fee_payer: H160::zero(),
        };

        let signer = state_ref.signer().get().clone();

        (signer, mint_order)
    };

    let signed_mint_order = mint_order
        .encode_and_sign(&signer)
        .await
        .map_err(|err| DepositError::MintOrderSign(format!("{err:?}")))?;

    Ok(signed_mint_order)
}

fn store_mint_order(
    state: &RefCell<State>,
    signed_mint_order: SignedMintOrder,
    eth_address: &H160,
    nonce: u32,
) {
    let mut state = state.borrow_mut();
    let sender_chain_id = state.erc20_chain_id();
    let sender = Id256::from_evm_address(eth_address, sender_chain_id);
    state
        .mint_orders_mut()
        .push(sender, nonce, signed_mint_order);

    log::trace!("Mint order added");
}

async fn send_mint_order(
    state: &RefCell<State>,
    mint_order: SignedMintOrder,
) -> Result<H256, DepositError> {
    log::info!("Sending mint transaction");

    let signer = state.borrow().signer().get().clone();
    let sender = signer
        .get_address()
        .await
        .map_err(|err| DepositError::MintOrderSign(format!("{err:?}")))?;

    let (evm_info, evm_params) = {
        let state = state.borrow();

        let evm_info = state.get_evm_info();
        let evm_params = state
            .get_evm_params()
            .clone()
            .ok_or(DepositError::NotInitialized(
                "Bridge must be initialized first".to_string(),
            ))?;

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
        .map_err(|err| DepositError::MintOrderSign(format!("{err:?}")))?;

    tx.r = signature.r.0;
    tx.s = signature.s.0;
    tx.v = signature.v.0;
    tx.hash = tx.hash();

    let client = evm_info.link.get_client();
    let id = client
        .send_raw_transaction(tx)
        .await
        .map_err(|err| DepositError::Evm(format!("{err:?}")))?;

    state.borrow_mut().update_evm_params(|p| {
        if let Some(params) = p.as_mut() {
            params.nonce += 1;
        }
    });

    log::info!("Mint transaction sent");

    Ok(id.into())
}

/// Swap an ERC20 for a BRC20.
///
/// This burns an ERC20 and transfers the BRC20.
pub async fn erc20_to_brc20(
    state: &RefCell<State>,
    request_id: u32,
    brc20_iid: String,
    dst_addr: &str,
) -> Result<Brc20InscribeStatus, WithdrawError> {
    let tx_ids = withdraw_brc20(state, request_id, &brc20_iid, dst_addr).await?;
    Ok(Brc20InscribeStatus { tx_ids })
}

async fn withdraw_brc20(
    state: &RefCell<State>,
    request_id: u32,
    brc20_iid: &str,
    dst_addr: &str,
) -> Result<InscribeTransactions, WithdrawError> {
    if !state.borrow().has_brc20(brc20_iid) {
        return Err(WithdrawError::NoSuchInscription(format!(
            "Specified BRC20 inscription ID ({}) not found",
            brc20_iid
        )));
    }

    let inscription = state
        .borrow()
        .inscriptions()
        .fetch_by_id(brc20_iid)
        .encode()
        .map_err(|err| WithdrawError::InvalidInscription(err.to_string()))?;

    let (network, ecdsa_signer) = {
        let state = state.borrow();
        let signer = IcBtcSigner::new(state.master_key(), state.btc_network());
        (state.ic_btc_network(), signer)
    };

    log::info!(
        "Transferring requested BRC20 token to {} with request id {}",
        dst_addr,
        request_id
    );

    let result = Inscriber::inscribe(
        Protocol::Brc20,
        inscription,
        &H160::default(),
        dst_addr.to_string(),
        dst_addr.to_string(),
        None,
        ecdsa_signer,
        network,
    )
    .await
    .map_err(|e| WithdrawError::InscriptionTransfer(e.to_string()));

    let mut state = state.borrow_mut();
    if result.is_ok() {
        let brc20_iid = InscriptionId::parse_from_str(brc20_iid)
            .expect("Failed to parse InscriptionId from string");

        state
            .inscriptions_mut()
            .remove(brc20_iid)
            .map_err(WithdrawError::NoSuchInscription)?;

        state.burn_requests_mut().set_transferred(request_id);
        state.burn_requests_mut().remove(request_id);
    } else {
        log::error!("Failed to transfer BRC20 for request {}", request_id);
    }

    result
}
