use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use bitcoin::consensus::Encodable;
use bitcoin::{Address, FeeRate, Transaction, Txid};
use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    bitcoin_get_current_fee_percentiles, bitcoin_get_utxos, bitcoin_send_transaction,
    BitcoinNetwork, GetCurrentFeePercentilesRequest, GetUtxosRequest, GetUtxosResponse, Outpoint,
    SendTransactionRequest, Utxo,
};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use ic_stable_structures::CellStructure;
use minter_did::id256::Id256;
use minter_did::order::{MintOrder, SignedMintOrder};
use ord_rs::wallet::{CreateEdictTxArgs, ScriptType, TxInputInfo};
use ord_rs::OrdTransactionBuilder;
use ordinals::{RuneId, SpacedRune};
use serde::Deserialize;

use crate::interface::{DepositError, Erc20MintStatus, OutputResponse, WithdrawError};
use crate::key::{get_deposit_address, get_derivation_path_ic};
use crate::ledger::UtxoKey;
use crate::rune_info::{RuneInfo, RuneName};
use crate::state::State;

const DEFAULT_REGTEST_FEE: u64 = 10_000;
const CYCLES_PER_HTTP_REQUEST: u128 = 500_000_000;
static NONCE: AtomicU32 = AtomicU32::new(0);

pub async fn deposit(
    state: Rc<RefCell<State>>,
    eth_address: &H160,
) -> Result<Vec<Erc20MintStatus>, DepositError> {
    log::trace!("Requested deposit for eth address: {eth_address}");

    let deposit_address =
        get_deposit_address(&state, eth_address).expect("Failed to get deposit address");
    let utxo_response: GetUtxosResponse = get_utxos(&state, &deposit_address).await?;

    if utxo_response.utxos.is_empty() {
        log::trace!("No utxos were found for address {deposit_address}");
        return Err(DepositError::NotingToDeposit);
    }

    log::trace!(
        "Found {} utxos at the address {}",
        utxo_response.utxos.len(),
        deposit_address
    );

    validate_utxo_confirmations(&state, &utxo_response)?;
    validate_utxo_btc_amount(&state, &utxo_response)?;

    let rune_amounts = get_rune_amounts(&state, &utxo_response.utxos).await?;
    if rune_amounts.is_empty() {
        return Err(DepositError::NoRunesToDeposit);
    }

    let Some(rune_info_amounts) = fill_rune_infos(&state, &rune_amounts).await else {
        return Err(DepositError::Unavailable(
            "Ord indexer is in invalid state".to_string(),
        ));
    };

    let sender = Id256::from_evm_address(eth_address, state.borrow().erc20_chain_id());

    let mut results = vec![];
    for (rune_info, amount) in rune_info_amounts {
        let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
        let mint_order = create_mint_order(&state, eth_address, amount, rune_info, nonce).await?;

        state
            .borrow_mut()
            .mint_orders_mut()
            .push(sender, nonce, mint_order);
        state.borrow_mut().ledger_mut().deposit(
            &utxo_response.utxos,
            &deposit_address,
            get_derivation_path_ic(eth_address),
        );

        let result = match send_mint_order(&state, mint_order).await {
            Ok(tx_id) => Erc20MintStatus::Minted { amount, tx_id },
            Err(err) => {
                log::warn!("Failed to send mint order: {err:?}");
                Erc20MintStatus::Signed(Box::new(mint_order))
            }
        };

        results.push(result);
    }

    Ok(results)
}

async fn fill_rune_infos(
    state: &RefCell<State>,
    rune_amounts: &HashMap<RuneName, u128>,
) -> Option<Vec<(RuneInfo, u128)>> {
    match fill_rune_infos_from_state(state, rune_amounts) {
        Some(v) => Some(v),
        None => fill_rune_infos_from_indexer(state, rune_amounts).await,
    }
}

fn fill_rune_infos_from_state(
    state: &RefCell<State>,
    rune_amounts: &HashMap<RuneName, u128>,
) -> Option<Vec<(RuneInfo, u128)>> {
    let state = state.borrow();
    let runes = state.runes();
    let mut infos = vec![];
    for (rune_name, amount) in rune_amounts {
        infos.push((*runes.get(rune_name)?, *amount));
    }

    Some(infos)
}

async fn fill_rune_infos_from_indexer(
    state: &RefCell<State>,
    rune_amounts: &HashMap<RuneName, u128>,
) -> Option<Vec<(RuneInfo, u128)>> {
    let rune_list = get_rune_list(state).await.ok()?;
    let runes: HashMap<RuneName, RuneInfo> = rune_list
        .iter()
        .map(|(rune_id, spaced_rune, decimals)| {
            (
                spaced_rune.rune.into(),
                RuneInfo {
                    name: spaced_rune.rune.into(),
                    decimals: *decimals,
                    block: rune_id.block,
                    tx: rune_id.tx,
                },
            )
        })
        .collect();
    let mut infos = vec![];
    for (rune_name, amount) in rune_amounts {
        match runes.get(rune_name) {
            Some(v) => infos.push((*v, *amount)),
            None => {
                log::error!("Ord indexer didn't return a rune information for rune {rune_name} that was present in an UTXO");
                return None;
            }
        }
    }

    state.borrow_mut().update_rune_list(runes);

    Some(infos)
}

pub async fn withdraw(
    state: &RefCell<State>,
    amount: u128,
    rune_id: RuneId,
    address: Address,
) -> Result<Txid, WithdrawError> {
    let (utxo_keys, current_utxos) = state.borrow().ledger().load_unspent_utxos();
    let tx =
        build_withdraw_transaction(state, amount, address.clone(), rune_id, current_utxos).await?;
    send_tx(state, &tx).await?;

    // mark all utxos as used
    utxo_keys.into_iter().for_each(|key| {
        state
            .borrow_mut()
            .ledger_mut()
            .mark_as_used(key, address.clone());
    });

    Ok(tx.txid())
}

pub async fn build_withdraw_transaction(
    state: &RefCell<State>,
    amount: u128,
    address: Address,
    rune: RuneId,
    inputs: Vec<TxInputInfo>,
) -> Result<Transaction, WithdrawError> {
    if inputs.is_empty() {
        return Err(WithdrawError::NoInputs);
    }

    let public_key = state.borrow().public_key();
    let wallet = state.borrow().wallet();

    let builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, wallet);

    let change_address = get_change_address(state)?;
    let rune_change_address = change_address.clone();

    let fee_rate = get_fee_rate(state).await?;

    let args = CreateEdictTxArgs {
        rune,
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
    let network = state.borrow().ic_btc_network();
    let args = GetCurrentFeePercentilesRequest { network };
    let response = bitcoin_get_current_fee_percentiles(args)
        .await
        .map_err(|err| {
            log::error!("Failed to get current fee rate: {err:?}");
            WithdrawError::FeeRateRequest
        })?
        .0;

    let middle_percentile = if response.is_empty() {
        match network {
            BitcoinNetwork::Regtest => DEFAULT_REGTEST_FEE,
            _ => {
                log::error!("Empty response for fee rate request");
                return Err(WithdrawError::FeeRateRequest);
            }
        }
    } else {
        response[response.len() / 2]
    };

    log::trace!("Received fee rate percentiles: {response:?}");

    log::info!("Using fee rate {}", middle_percentile / 1000);

    FeeRate::from_sat_per_vb(middle_percentile / 1000).ok_or_else(|| {
        log::error!("Invalid fee rate received from IC: {middle_percentile}");
        WithdrawError::FeeRateRequest
    })
}

async fn send_tx(state: &RefCell<State>, transaction: &Transaction) -> Result<(), WithdrawError> {
    log::trace!(
        "Sending transaction {} to the bitcoin adapter",
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

    // filter out spent utxos and spent utxos from the ledger
    filter_out_spent_utxos(state, &mut response)
        .into_iter()
        .for_each(|utxo_key| {
            state.borrow_mut().ledger_mut().remove_spent_utxo(&utxo_key);
        });

    Ok(response)
}

/// Filters out utxos that are already spent in the ledger.
/// Returns the filtered utxox.
fn filter_out_spent_utxos(
    state: &RefCell<State>,
    get_utxos_response: &mut GetUtxosResponse,
) -> Vec<UtxoKey> {
    let used_utxos = state.borrow().ledger().load_used_utxos();
    println!("Used utxos: {:?}", used_utxos);
    let spent_utxos = used_utxos
        .into_iter()
        .filter(|(used_utxo_key, _)| {
            !get_utxos_response.utxos.iter().any(|owner_utxo| {
                owner_utxo.outpoint.txid == used_utxo_key.tx_id
                    && owner_utxo.outpoint.vout == used_utxo_key.vout
            })
        })
        .map(|(used_utxo_key, _)| used_utxo_key)
        .collect::<Vec<_>>();
    println!("Spent utxos: {:?}", spent_utxos);

    get_utxos_response
        .utxos
        .retain(|utxo| !spent_utxos.contains(&UtxoKey::from(&utxo.outpoint)));

    spent_utxos
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
        log::trace!(
            "Current utxo confirmations {} satisfies minimum {}. Proceeding.",
            utxo_min_confirmations,
            min_confirmations
        );
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

    log::trace!(
        "Input utxo BTC amount is {}, which satisfies minimum of {}",
        received_amount,
        min_amount
    );

    Ok(())
}

async fn get_rune_amounts(
    state: &RefCell<State>,
    utxos: &[Utxo],
) -> Result<HashMap<RuneName, u128>, DepositError> {
    log::trace!("Requesting rune balance for given inputs");

    let mut amounts = HashMap::new();
    for utxo in utxos {
        for (rune_name, amount) in get_tx_rune_amounts(state, utxo).await? {
            *amounts.entry(rune_name).or_default() += amount;
        }
    }

    log::trace!("Total rune balances for input utxos: {amounts:?}");

    Ok(amounts)
}

pub async fn get_rune_list(
    state: &RefCell<State>,
) -> Result<Vec<(RuneId, SpacedRune, u8)>, DepositError> {
    #[derive(Debug, Clone, Deserialize)]
    struct RuneInfo {
        spaced_rune: SpacedRune,
        divisibility: u8,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RunesResponse {
        entries: Vec<(RuneId, RuneInfo)>,
    }

    const MAX_RESPONSE_BYTES: u64 = 10_000;

    // todo: AFAIK this endpoint will return first 50 entries. Need to figure out how to use
    // pagination with this api.
    // https://infinityswap.atlassian.net/browse/EPROD-854
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
        .map(|(rune_id, info)| (rune_id, info.spaced_rune, info.divisibility))
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

async fn get_tx_rune_amounts(
    state: &RefCell<State>,
    utxo: &Utxo,
) -> Result<HashMap<RuneName, u128>, DepositError> {
    let response = get_tx_outputs(state, utxo).await?;
    let amounts = response
        .runes
        .iter()
        .map(|(spaced_rune, pile)| (spaced_rune.rune.into(), pile.amount))
        .collect();

    log::trace!(
        "Received rune balances for utxo {}: {:?}",
        hex::encode(&utxo.outpoint.txid),
        amounts
    );

    Ok(amounts)
}

async fn create_mint_order(
    state: &RefCell<State>,
    eth_address: &H160,
    amount: u128,
    rune_info: RuneInfo,
    nonce: u32,
) -> Result<SignedMintOrder, DepositError> {
    log::trace!("preparing mint order");

    let (signer, mint_order) = {
        let state_ref = state.borrow();

        let sender_chain_id = state_ref.btc_chain_id();
        let sender = Id256::from_evm_address(eth_address, sender_chain_id);
        let src_token = Id256::from(rune_info.id());

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
            name: rune_info.name_array(),
            symbol: rune_info.symbol_array(),
            decimals: rune_info.decimals(),
            approve_spender: Default::default(),
            approve_amount: Default::default(),
            fee_payer: H160::default(),
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

    let client = evm_info.link.get_json_rpc_client();
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
    use std::str::FromStr;

    use ic_exports::ic_kit::MockContext;

    use super::*;
    use crate::canister::get_state;

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

    #[test]
    fn test_should_remove_spent_utxo() {
        MockContext::new().inject();

        // utxos used; the last two are spent
        let spent_utxos: Vec<Utxo> = vec![
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xde; 32],
                    vout: 0,
                },
                value: 0,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xde; 32],
                    vout: 1,
                },
                value: 0,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xca; 32],
                    vout: 0,
                },
                value: 0,
                height: 0,
            },
        ];

        // In response we put one utxo that is not spent, but is deposited and one not tracked.
        let response_utxos = vec![
            spent_utxos[0].clone(),
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xbb; 32],
                    vout: 0,
                },
                value: 0,
                height: 0,
            },
        ];
        let mut response = GetUtxosResponse {
            utxos: response_utxos.clone(),
            tip_block_hash: vec![],
            tip_height: 0,
            next_page: None,
        };
        // put used utxo in the ledger
        let address = Address::from_str("bc1quyjp8qxkdc22cej962xaydd5arm7trwtcnkzks")
            .unwrap()
            .assume_checked();
        let state = get_state();
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(&spent_utxos, &address, vec![]);
        // mark utxos as used
        state
            .borrow_mut()
            .ledger_mut()
            .mark_as_used(UtxoKey::from(&spent_utxos[0].outpoint), address.clone());
        state
            .borrow_mut()
            .ledger_mut()
            .mark_as_used(UtxoKey::from(&spent_utxos[1].outpoint), address.clone());
        state
            .borrow_mut()
            .ledger_mut()
            .mark_as_used(UtxoKey::from(&spent_utxos[2].outpoint), address);

        // filter
        let spent_utxos = filter_out_spent_utxos(&state, &mut response);

        // check response
        assert_eq!(response.utxos.len(), 2);
        assert_eq!(response.utxos[0].outpoint.txid, vec![0xde; 32]);
        assert_eq!(response.utxos[0].outpoint.vout, 0);
        assert_eq!(response.utxos[1].outpoint.txid, vec![0xbb; 32]);
        assert_eq!(response.utxos[1].outpoint.vout, 0);
        // check spent utxos
        assert_eq!(spent_utxos.len(), 2);
        assert_eq!(spent_utxos[0].tx_id.to_vec(), vec![0xca; 32]);
        assert_eq!(spent_utxos[0].vout, 0);
        assert_eq!(spent_utxos[1].tx_id.to_vec(), vec![0xde; 32]);
        assert_eq!(spent_utxos[1].vout, 1);
    }
}
