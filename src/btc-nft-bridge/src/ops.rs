use core::sync::atomic::Ordering;
use std::cell::RefCell;

use bitcoin::Txid;
use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_stable_structures::CellStructure;
use inscriber::ops as Inscriber;
use minter_contract_utils::erc721_mint_order::{MintOrder, SignedMintOrder};
use minter_did::id256::Id256;
use ord_rs::inscription::nft::id::NftId;

use crate::constant::NONCE;
use crate::interface::bridge_api::{BridgeError, NftInscribeStatus, NftMintError, NftMintStatus};
use crate::interface::get_deposit_address;
use crate::interface::store::NftInfo;
use crate::rpc;
use crate::state::State;

/// Swap a BTC-NFT for an ERC721.
///
/// This burns a BTC-NTF and mints an equivalent ERC721.
pub async fn nft_to_erc721(
    state: &RefCell<State>,
    eth_address: H160,
    nft_id: NftId,
    holder_btc_addr: String,
) -> Result<NftMintStatus, NftMintError> {
    let nft = rpc::fetch_nft_token_details(state, nft_id, holder_btc_addr)
        .await
        .map_err(|e| NftMintError::InvalidNft(e.to_string()))?;

    let reveal_tx = rpc::fetch_reveal_transaction(state, &nft.tx_id)
        .await
        .map_err(|e| NftMintError::NftBridge(e.to_string()))?;

    rpc::parse_and_validate_inscription(reveal_tx)
        .await
        .map_err(|e| NftMintError::InvalidNft(e.to_string()))?;

    state.borrow_mut().inscriptions_mut().insert(nft);

    /*
    let (amount, tick) = rpc::get_brc20_data(&nft);
    // Set the token symbol using the tick (symbol) from the BRC20
    state
        .borrow_mut()
        .set_token_symbol(tick)
        .map_err(|e| NftMintError::Brc20Bridge(e.to_string()))?;
     */

    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);

    log::info!("Minting an ERC721 token with nft-id: {nft_id}");
    mint_erc721(state, eth_address, nft_id, nonce).await
}

pub async fn mint_erc721(
    state: &RefCell<State>,
    eth_address: H160,
    nft_id: NftId,
    nonce: u32,
) -> Result<NftMintStatus, NftMintError> {
    let mint_order = prepare_mint_order(
        state,
        eth_address.clone(),
        nonce,
        nft_id.clone().to_string(),
    )
    .await?;
    store_mint_order(state, mint_order.clone(), &eth_address, nonce);

    Ok(match send_mint_order(state, mint_order.clone()).await {
        Ok(tx_id) => NftMintStatus::Minted {
            id: nft_id.into(),
            tx_id,
        },
        Err(err) => {
            log::warn!("Failed to send mint order: {err:?}");
            NftMintStatus::Signed(Box::new(mint_order))
        }
    })
}

async fn prepare_mint_order(
    state: &RefCell<State>,
    eth_address: H160,
    nonce: u32,
    token_uri: String,
) -> Result<SignedMintOrder, NftMintError> {
    log::info!("preparing mint order");

    let (signer, mint_order) = {
        let state_ref = state.borrow();

        let sender_chain_id = state_ref.btc_chain_id();
        let sender = Id256::from_evm_address(&eth_address, sender_chain_id);
        let src_token = Id256::from(&ic_exports::ic_kit::ic::id());

        let recipient_chain_id = state_ref.erc721_chain_id();

        let mint_order = MintOrder {
            sender,
            src_token,
            recipient: eth_address,
            dst_token: H160::default(),
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name: state_ref.token_name(),
            symbol: state_ref.token_symbol(),
            approve_spender: Default::default(),
            token_uri,
        };

        let signer = state_ref.signer().get().clone();

        (signer, mint_order)
    };

    let signed_mint_order = mint_order
        .encode_and_sign(&signer)
        .await
        .map_err(|err| NftMintError::Sign(format!("{err:?}")))?;

    Ok(signed_mint_order)
}

fn store_mint_order(
    state: &RefCell<State>,
    signed_mint_order: SignedMintOrder,
    eth_address: &H160,
    nonce: u32,
) {
    let mut state = state.borrow_mut();
    let sender_chain_id = state.btc_chain_id();
    let sender = Id256::from_evm_address(eth_address, sender_chain_id);
    state
        .mint_orders_mut()
        .push(sender, nonce, signed_mint_order);

    log::trace!("Mint order added");
}

async fn send_mint_order(
    state: &RefCell<State>,
    mint_order: SignedMintOrder,
) -> Result<H256, NftMintError> {
    log::info!("Sending mint transaction");

    let signer = state.borrow().signer().get().clone();
    let sender = signer
        .get_address()
        .await
        .map_err(|err| NftMintError::Sign(format!("{err:?}")))?;

    let (evm_info, evm_params) = {
        let state = state.borrow();

        let evm_info = state.get_evm_info();
        let evm_params = state
            .get_evm_params()
            .clone()
            .ok_or(NftMintError::NotInitialized(
                "Bridge must be initialized first".to_string(),
            ))?;

        (evm_info, evm_params)
    };

    let mut tx = minter_contract_utils::erc721_bridge_api::mint_transaction(
        sender.0,
        evm_info.bridge_contract.0,
        evm_params.nonce.into(),
        evm_params.gas_price.into(),
        mint_order.0.to_vec(),
        evm_params.chain_id as _,
    );

    let signature = signer
        .sign_transaction(&(&tx).into())
        .await
        .map_err(|err| NftMintError::Sign(format!("{err:?}")))?;

    tx.r = signature.r.0;
    tx.s = signature.s.0;
    tx.v = signature.v.0;
    tx.hash = tx.hash();

    let client = evm_info.link.get_client();
    let id = client
        .send_raw_transaction(tx)
        .await
        .map_err(|err| NftMintError::Evm(format!("{err:?}")))?;

    state.borrow_mut().update_evm_params(|p| {
        if let Some(params) = p.as_mut() {
            params.nonce += 1;
        }
    });

    log::info!("Mint transaction sent");

    Ok(id.into())
}

/// Swap an NFT on Eth for a BTC-NFT.
///
/// This burns an Nft and inscribes an equivalent on BTC.
pub async fn erc721_to_nft(
    state: &RefCell<State>,
    request_id: u32,
    nft_id: NftId,
    dst_addr: &str,
) -> Result<NftInscribeStatus, BridgeError> {
    let (network, derivation_path) = {
        (
            state.borrow().ic_btc_network(),
            state.borrow().derivation_path(None),
        )
    };

    let bridge_addr = get_deposit_address(network, derivation_path).await;

    let nft_info = rpc::fetch_nft_token_details(state, nft_id, bridge_addr)
        .await
        .map_err(|e| NftMintError::InvalidNft(e.to_string()))?;

    let tx_id = withdraw_nft(state, request_id, nft_info, dst_addr)
        .await
        .map_err(|e| BridgeError::Erc721Burn(e.to_string()))?;

    Ok(NftInscribeStatus {
        tx_id: tx_id.to_string(),
    })
}

async fn withdraw_nft(
    state: &RefCell<State>,
    request_id: u32,
    nft: NftInfo,
    dst_addr: &str,
) -> Result<Txid, BridgeError> {
    if !state.borrow().has_nft(&nft.tx_id) {
        return Err(BridgeError::Erc721Burn(format!(
            "Specified tx ID ({}) not associated with any BTC inscription",
            nft.tx_id
        )));
    }

    rpc::fetch_reveal_transaction(state, &nft.tx_id)
        .await
        .map_err(|e| BridgeError::GetTransactionById(e.to_string()))?;

    let (network, derivation_path) = {
        let state = state.borrow();
        (state.ic_btc_network(), state.derivation_path(None))
    };

    log::info!(
        "Transferring requested NFT to {} with request id {}",
        dst_addr,
        request_id
    );

    // transfer the UTXO to the destination address
    let result = Inscriber::transfer_utxo(
        (&nft).into(),
        dst_addr.to_string(),
        dst_addr.to_string(),
        None,
        derivation_path,
        network,
    )
    .await
    .map_err(|e| BridgeError::Erc721Burn(e.to_string()));

    let mut state = state.borrow_mut();
    if result.is_ok() {
        state
            .inscriptions_mut()
            .remove(nft.tx_id.to_string())
            .map_err(|e| BridgeError::Erc721Burn(e.to_string()))?;

        state.burn_requests_mut().set_transferred(request_id);
        state.burn_requests_mut().remove(request_id);
    } else {
        log::error!("Failed to transfer BRC20 for request {}", request_id);
    }

    result
}
