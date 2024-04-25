use crate::interface::{DepositError, Erc20MintStatus, OutputResponse, WithdrawError};
use crate::key::{get_deposit_address, get_derivation_path_ic};
use crate::ledger::StoredUtxo;
use crate::state::State;
use bitcoin::consensus::Encodable;
use bitcoin::hashes::Hash;
use bitcoin::{Address, FeeRate, Transaction, Txid};
use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    bitcoin_get_current_fee_percentiles, bitcoin_get_utxos, bitcoin_send_transaction,
    GetCurrentFeePercentilesRequest, GetUtxosRequest, GetUtxosResponse, Outpoint,
    SendTransactionRequest, Utxo,
};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use ic_exports::ic_kit::ic;
use ic_stable_structures::CellStructure;
use minter_did::id256::Id256;
use minter_did::order::{MintOrder, SignedMintOrder};
use ord_rs::wallet::{CreateEdictTxArgs, ScriptType};
use ord_rs::OrdTransactionBuilder;
use ordinals::{RuneId, SpacedRune};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

const CYCLES_PER_HTTP_REQUEST: u128 = 500_000_000;
static NONCE: AtomicU32 = AtomicU32::new(0);

pub async fn deposit(
    state: Rc<RefCell<State>>,
    eth_address: &H160,
) -> Result<Erc20MintStatus, DepositError> {
    log::trace!("Requested deposit for eth address: {eth_address}");

    let deposit_address =
        get_deposit_address(&state, eth_address).expect("Failed to get deposit address");
    let utxo_response: GetUtxosResponse = get_utxos(&state, &deposit_address).await?;

    if utxo_response.utxos.is_empty() {
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
        .push(sender, nonce, mint_order);
    state.borrow_mut().ledger_mut().deposit(
        &utxo_response.utxos,
        &deposit_address,
        get_derivation_path_ic(eth_address),
    );

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

pub async fn withdraw(
    state: &RefCell<State>,
    amount: u128,
    address: Address,
) -> Result<Txid, WithdrawError> {
    let current_utxos = state.borrow().ledger().load_all();
    let tx = build_withdraw_transaction(state, amount, address, current_utxos).await?;
    send_tx(state, &tx).await?;

    Ok(tx.txid())
}

pub async fn build_withdraw_transaction(
    state: &RefCell<State>,
    amount: u128,
    address: Address,
    inputs: Vec<StoredUtxo>,
) -> Result<Transaction, WithdrawError> {
    if inputs.is_empty() {
        return Err(WithdrawError::NoInputs);
    }

    if !inputs
        .iter()
        .all(|input| input.derivation_path == inputs[0].derivation_path)
    {
        todo!();
    }

    let derivation_path = &inputs[0].derivation_path;
    let public_key = state.borrow().der_public_key(derivation_path);
    let signer = state.borrow().wallet(derivation_path.clone());

    let builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, signer);

    let change_address = get_change_address(state)?;
    let rune_change_address = change_address.clone();

    let fee_rate = get_fee_rate(state).await?;

    let inputs = inputs.into_iter().map(|v| v.tx_input_info).collect();
    let args = CreateEdictTxArgs {
        rune: state.borrow().rune_id(),
        inputs,
        destination: address,
        change_address,
        rune_change_address,
        amount,
        fee_rate,
    };
    let unsigned_tx = builder.create_edict_transaction(&args).map_err(|err| {
        log::warn!("Failed to create withdraw transaction: {err:?}");
        WithdrawError::TransactionCreation
    })?;
    let signed_tx = builder
        .sign_transaction(&unsigned_tx, &args.inputs)
        .await
        .map_err(|err| {
            log::error!("Failed to sign withdraw transaction: {err:?}");
            WithdrawError::TransactionSigning
        })?;

    Ok(signed_tx)
}

fn get_change_address(state: &RefCell<State>) -> Result<Address, WithdrawError> {
    get_deposit_address(state, &H160::default()).map_err(|err| {
        log::error!("Failed to get change address: {err:?}");
        WithdrawError::ChangeAddress
    })
}

pub async fn get_fee_rate(state: &RefCell<State>) -> Result<FeeRate, WithdrawError> {
    let args = GetCurrentFeePercentilesRequest {
        network: state.borrow().ic_btc_network(),
    };
    let response = bitcoin_get_current_fee_percentiles(args)
        .await
        .map_err(|err| {
            log::error!("Failed to get current fee rate: {err:?}");
            WithdrawError::FeeRateRequest
        })?
        .0;

    if response.is_empty() {
        log::error!("Empty response for fee rate request");
        return Err(WithdrawError::FeeRateRequest);
    }

    log::trace!("Received fee rate percentiles: {response:?}");

    let middle_percentile = &response[response.len() / 2];

    log::info!("Using fee rate {}", middle_percentile / 1000);

    FeeRate::from_sat_per_vb(middle_percentile / 1000).ok_or_else(|| {
        log::error!("Invalid fee rate received from IC: {middle_percentile}");
        WithdrawError::FeeRateRequest
    })
}

async fn send_tx(state: &RefCell<State>, transaction: &Transaction) -> Result<(), WithdrawError> {
    log::trace!(
        "Sending transaction {} to the bitcoin addapter",
        transaction.txid()
    );

    let mut serialized = vec![];
    transaction
        .consensus_encode(&mut serialized)
        .map_err(|err| {
            log::error!("Failed to serialize transaction: {err:?}");
            WithdrawError::TransactionSerialization
        })?;

    log::trace!(
        "Serialized transaction {}: {}",
        transaction.txid(),
        hex::encode(&serialized)
    );

    let request = SendTransactionRequest {
        transaction: serialized,
        network: state.borrow().ic_btc_network(),
    };
    bitcoin_send_transaction(request).await.map_err(|err| {
        log::error!("Failed to send transaction: {err:?}");
        WithdrawError::TransactionSending
    })?;

    log::trace!("Transaction {} sent to the adapter", transaction.txid());

    Ok(())
}

pub async fn get_utxos(
    state: &RefCell<State>,
    address: &Address,
) -> Result<GetUtxosResponse, DepositError> {
    let args = GetUtxosRequest {
        address: address.to_string(),
        network: state.borrow().ic_btc_network(),
        filter: None,
    };

    log::trace!("Requesting UTXO list for address {address}");

    let mut response = bitcoin_get_utxos(args)
        .await
        .map(|value| value.0)
        .map_err(|err| {
            DepositError::Unavailable(format!(
                "Unexpected response from management canister: {err:?}"
            ))
        })?;

    log::trace!("Got UTXO list result for address {address}:");
    log::trace!("{response:?}");

    filter_out_used_utxos(state, &mut response);

    Ok(response)
}

fn filter_out_used_utxos(state: &RefCell<State>, get_utxos_response: &mut GetUtxosResponse) {
    let existing = state.borrow().ledger().load_all();

    get_utxos_response.utxos.retain(|utxo| {
        !existing.iter().any(|v| {
            v.tx_input_info.outpoint.txid.as_byte_array()[..] == utxo.outpoint.txid
                && v.tx_input_info.outpoint.vout == utxo.outpoint.vout
        })
    })
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

pub async fn get_rune_list(
    state: &RefCell<State>,
) -> Result<Vec<(RuneId, SpacedRune)>, DepositError> {
    #[derive(Debug, Clone, Deserialize)]
    struct RuneInfo {
        spaced_rune: SpacedRune,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RunesResponse {
        entries: Vec<(RuneId, RuneInfo)>,
    }

    const MAX_RESPONSE_BYTES: u64 = 10_000;

    let indexer_url = state.borrow().indexer_url();
    let url = format!("{indexer_url}/runes");

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

    let response: RunesResponse = serde_json::from_slice(&result.body).map_err(|err| {
        log::error!("Failed to get rune balance from the indexer: {err:?}");
        DepositError::Unavailable(format!("Unexpected response from indexer: {err:?}"))
    })?;

    Ok(response
        .entries
        .into_iter()
        .map(|(rune_id, info)| (rune_id, info.spaced_rune))
        .collect())
}

pub async fn get_tx_outputs(
    state: &RefCell<State>,
    utxo: &Utxo,
) -> Result<OutputResponse, DepositError> {
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

    serde_json::from_slice(&result.body).map_err(|err| {
        log::error!("Failed to get rune balance from the indexer: {err:?}");
        DepositError::Unavailable(format!("Unexpected response from indexer: {err:?}"))
    })
}

async fn get_tx_rune_amount(state: &RefCell<State>, utxo: &Utxo) -> Result<u128, DepositError> {
    let response = get_tx_outputs(state, utxo).await?;
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
        let sender = Id256::from_evm_address(eth_address, sender_chain_id);
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
        hex::encode(outpoint.txid.iter().copied().rev().collect::<Vec<u8>>()),
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
