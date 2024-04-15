use crate::interface::{DepositError, DepositResponse, Erc20MintStatus};
use crate::key::get_deposit_address;
use crate::state::State;
use bitcoin::Address;
use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    bitcoin_get_utxos, GetUtxosRequest, GetUtxosResponse, Outpoint, Utxo,
};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use ic_exports::ic_kit::ic;
use ic_stable_structures::CellStructure;
use minter_did::id256::Id256;
use minter_did::order::{MintOrder, SignedMintOrder};
use ordinals::{Pile, SpacedRune};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

const CYCLES_PER_HTTP_REQUEST: u128 = 100_000_000;
static NONCE: AtomicU32 = AtomicU32::new(0);

pub async fn deposit(
    state: Rc<RefCell<State>>,
    eth_address: &H160,
) -> Result<Erc20MintStatus, DepositError> {
    log::trace!("Requested deposit for eth address: {eth_address}");

    let deposit_address =
        get_deposit_address(&state, eth_address).expect("Failed to get deposit address");
    let utxo_response: GetUtxosResponse = get_utxos(&state, &deposit_address).await?;

    if utxo_response.utxos.len() == 0 {
        log::trace!("No utxos were found for address {deposit_address}");
        return Err(DepositError::NotingToDeposit);
    }

    validate_utxo_confirmations(&state, &utxo_response)?;
    validate_utxo_btc_amount(&state, &utxo_response)?;

    let rune_amount: u128 = get_rune_amount(&state, &utxo_response.utxos).await?;
    if rune_amount == 0 {
        return Err(DepositError::NoRunesToDeposit);
    }

    let sender = Id256::from_evm_address(eth_address, state.borrow().erc20_chain_id());
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
    let mint_order = create_mint_order(&state, eth_address, rune_amount, nonce).await?;

    state
        .borrow_mut()
        .mint_orders_mut()
        .push(sender, nonce, mint_order.clone());
    state
        .borrow_mut()
        .ledger_mut()
        .deposit(&utxo_response.utxos, rune_amount);

    Ok(match send_mint_order(&state, mint_order).await {
        Ok(tx_id) => Erc20MintStatus::Minted {
            amount: rune_amount,
            tx_id,
        },
        Err(err) => {
            log::warn!("Failed to send mint order: {err:?}");
            Erc20MintStatus::Signed(Box::new(mint_order))
        }
    })
}

async fn get_utxos(
    state: &RefCell<State>,
    address: &Address,
) -> Result<GetUtxosResponse, DepositError> {
    let args = GetUtxosRequest {
        address: address.to_string(),
        network: state.borrow().ic_btc_network(),
        filter: None,
    };

    log::trace!("Requesting UTXO list for address {address}");

    let response = bitcoin_get_utxos(args)
        .await
        .map(|value| value.0)
        .map_err(|err| {
            DepositError::Unavailable(format!(
                "Unexpected response from management canister: {err:?}"
            ))
        });

    log::trace!("Got UTXO list result for address {address}:");
    log::trace!("{response:?}");

    response
}

fn validate_utxo_confirmations(
    state: &RefCell<State>,
    utxo_info: &GetUtxosResponse,
) -> Result<(), DepositError> {
    let min_confirmations = state.borrow().min_confirmations();
    let utxo_min_confirmations = utxo_info
        .utxos
        .iter()
        .map(|utxo| utxo_info.tip_height - utxo.height + 1)
        .min()
        .unwrap_or_default();

    if min_confirmations > utxo_min_confirmations {
        Err(DepositError::Pending {
            min_confirmations,
            current_confirmations: utxo_min_confirmations,
        })
    } else {
        Ok(())
    }
}

fn validate_utxo_btc_amount(
    state: &RefCell<State>,
    utxo_info: &GetUtxosResponse,
) -> Result<(), DepositError> {
    let received_amount = utxo_info.utxos.iter().map(|utxo| utxo.value).sum();
    let min_amount = state.borrow().deposit_fee();

    if received_amount < min_amount {
        return Err(DepositError::NotEnoughBtc {
            received: received_amount,
            minimum: min_amount,
        });
    }

    Ok(())
}

async fn get_rune_amount(state: &RefCell<State>, utxos: &[Utxo]) -> Result<u128, DepositError> {
    log::trace!("Requesting rune balance for given inputs");

    let mut amount = 0;
    for utxo in utxos {
        amount += get_tx_rune_amount(state, utxo).await?;
    }

    log::trace!("Total rune balance for input utxos: {amount}");

    Ok(amount)
}

async fn get_tx_rune_amount(state: &RefCell<State>, utxo: &Utxo) -> Result<u128, DepositError> {
    const MAX_RESPONSE_BYTES: u64 = 10_000;

    let indexer_url = state.borrow().indexer_url();
    let outpoint = format_outpoint(&utxo.outpoint);
    let url = format!("{indexer_url}/output/{outpoint}");

    log::trace!("Requesting rune balance from url: {url}");

    let request_params = CanisterHttpRequestArgument {
        url,
        max_response_bytes: Some(MAX_RESPONSE_BYTES),
        method: HttpMethod::GET,
        headers: vec![HttpHeader {
            name: "Accept".to_string(),
            value: "application/json".to_string(),
        }],
        body: None,
        transform: None,
    };

    #[derive(Debug, Clone, Deserialize)]
    struct OutputResponse {
        address: String,
        #[serde(default)]
        runes: Vec<(SpacedRune, Pile)>,
        spent: bool,
    }

    let result = http_request(request_params, CYCLES_PER_HTTP_REQUEST)
        .await
        .map_err(|err| DepositError::Unavailable(format!("Indexer unavailable: {err:?}")))?
        .0;

    log::trace!(
        "Indexer responded with: {} {:?} BODY: {}",
        result.status,
        result.headers,
        String::from_utf8_lossy(&result.body)
    );

    let response: OutputResponse = serde_json::from_slice(&result.body).map_err(|err| {
        log::error!("Failed to get rune balance from the indexer: {err:?}");
        DepositError::Unavailable(format!("Unexpected response from indexer: {err:?}"))
    })?;

    let mut amount = 0;
    let self_rune_name = state.borrow().rune_name();
    for (rune_id, pile) in response.runes {
        if rune_id.rune.to_string() == self_rune_name {
            amount += pile.amount;
        }
    }

    log::trace!(
        "Rune balance for utxo {}:{}: {amount}",
        hex::encode(&utxo.outpoint.txid),
        utxo.outpoint.vout
    );

    Ok(amount)
}

async fn create_mint_order(
    state: &RefCell<State>,
    eth_address: &H160,
    amount: u128,
    nonce: u32,
) -> Result<SignedMintOrder, DepositError> {
    log::trace!("preparing mint order");

    let (signer, mint_order) = {
        let state_ref = state.borrow();

        let sender_chain_id = state_ref.btc_chain_id();
        let sender = Id256::from_evm_address(&eth_address, sender_chain_id);
        let src_token = Id256::from(&ic::id());

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

        (signer, mint_order)
    };

    let signed_mint_order = mint_order
        .encode_and_sign(&signer)
        .await
        .map_err(|err| DepositError::Sign(format!("{err:?}")))?;

    Ok(signed_mint_order)
}

async fn send_mint_order(
    state: &RefCell<State>,
    mint_order: SignedMintOrder,
) -> Result<H256, DepositError> {
    log::trace!("Sending mint transaction");

    let signer = state.borrow().signer().get().clone();
    let sender = signer
        .get_address()
        .await
        .map_err(|err| DepositError::Sign(format!("{err:?}")))?;

    let (evm_info, evm_params) = {
        let state = state.borrow();

        let evm_info = state.get_evm_info();
        let evm_params = state
            .get_evm_params()
            .clone()
            .ok_or(DepositError::NotInitialized)?;

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
        .map_err(|err| DepositError::Sign(format!("{err:?}")))?;

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

    log::trace!("Mint transaction sent");

    Ok(id.into())
}

fn format_outpoint(outpoint: &Outpoint) -> String {
    // For some reason IC management canister returns bytes of tx_id in reversed order. It is
    // probably related to the fact that WASM uses little endian, but I'm not sure about that.
    // Nevertheless, to get the correct tx_id string we need to reverse the bytes first.
    format!(
        "{}:{}",
        hex::encode(&outpoint.txid.iter().copied().rev().collect::<Vec<u8>>()),
        outpoint.vout
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ic_outpoint_formatting() {
        let outpoint = Outpoint {
            txid: vec![
                98, 63, 184, 185, 7, 50, 158, 17, 243, 185, 211, 103, 188, 117, 181, 151, 60, 123,
                6, 92, 153, 208, 7, 254, 73, 104, 37, 139, 72, 22, 74, 26,
            ],
            vout: 2,
        };

        let expected = "1a4a16488b256849fe07d0995c067b3c97b575bc67d3b9f3119e3207b9b83f62:2";
        assert_eq!(&format_outpoint(&outpoint)[..], expected);
    }
}
