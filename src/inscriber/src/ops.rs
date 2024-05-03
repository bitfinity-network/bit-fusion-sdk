use std::str::FromStr as _;

use bitcoin::{Address, OutPoint, Txid};
use ethers_core::types::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ord_rs::MultisigConfig;

use crate::interface::{
    Brc20TransferTransactions, InscribeError, InscribeResult, InscribeTransactions,
    InscriptionFees, Multisig, Protocol,
};
use crate::wallet::CanisterWallet;

/// Inscribes a message onto the Bitcoin blockchain using the given inscription
/// type.
pub async fn inscribe(
    inscription_type: Protocol,
    inscription: String,
    leftovers_address: String,
    dst_address: String,
    multisig_config: Option<Multisig>,
    derivation_path: Vec<Vec<u8>>,
    network: BitcoinNetwork,
) -> InscribeResult<InscribeTransactions> {
    let leftovers_address = get_address(leftovers_address, network)?;

    let dst_address = get_address(dst_address, network)?;

    let multisig_config = multisig_config.map(|m| MultisigConfig {
        required: m.required,
        total: m.total,
    });

    CanisterWallet::new(derivation_path, network)
        .inscribe(
            inscription_type,
            inscription,
            dst_address,
            leftovers_address,
            multisig_config,
        )
        .await
}

/// Inscribes and sends the inscribed sat from this canister to the given address.
pub async fn brc20_transfer(
    inscription: String,
    leftovers_address: String,
    dst_address: String,
    multisig_config: Option<Multisig>,
    derivation_path: Vec<Vec<u8>>,
    network: BitcoinNetwork,
) -> InscribeResult<Brc20TransferTransactions> {
    let leftovers_address = get_address(leftovers_address, network)?;
    let transfer_dst_address = get_address(dst_address, network)?;

    let wallet = CanisterWallet::new(derivation_path.clone(), network);
    let inscription_dst_address = wallet.get_bitcoin_address().await;
    let inscription_leftovers_address = inscription_dst_address.clone();

    let inscribe_txs = inscribe(
        Protocol::Brc20,
        inscription,
        inscription_dst_address.to_string(),
        inscription_leftovers_address.to_string(),
        multisig_config,
        derivation_path,
        network,
    )
    .await?;

    let (transfer_tx, leftover_amount) = wallet
        .transfer_utxo(
            Txid::from_str(&inscribe_txs.commit_tx).unwrap(),
            Txid::from_str(&inscribe_txs.reveal_tx).unwrap(),
            transfer_dst_address,
            leftovers_address,
            inscribe_txs.leftover_amount,
        )
        .await?;

    Ok(Brc20TransferTransactions {
        commit_tx: inscribe_txs.commit_tx,
        reveal_tx: inscribe_txs.reveal_tx,
        transfer_tx: transfer_tx.to_string(),
        leftover_amount,
    })
}

pub async fn transfer_utxo(
    outpoints: &[OutPoint],
    leftovers_address: Address,
    dst_address: Address,
    multisig_config: Option<Multisig>,
    derivation_path: Vec<Vec<u8>>,
    network: BitcoinNetwork,
) -> InscribeResult<Txid> {
    todo!();
}

/// Gets the Bitcoin address for the given derivation path.
pub async fn get_bitcoin_address(derivation_path: Vec<Vec<u8>>, network: BitcoinNetwork) -> String {
    CanisterWallet::new(derivation_path, network)
        .get_bitcoin_address()
        .await
        .to_string()
}

pub async fn get_inscription_fees(
    inscription_type: Protocol,
    inscription: String,
    multisig_config: Option<Multisig>,
    network: BitcoinNetwork,
) -> InscribeResult<InscriptionFees> {
    let multisig_config = multisig_config.map(|m| MultisigConfig {
        required: m.required,
        total: m.total,
    });

    CanisterWallet::new(vec![], network)
        .get_inscription_fees(inscription_type, inscription, multisig_config)
        .await
}

/// Returns the derivation path to use for signing/verifying based on the caller principal or provided address.
#[inline]
pub(crate) fn derivation_path(address: Option<H160>) -> Vec<Vec<u8>> {
    let caller_principal = ic_exports::ic_cdk::caller().as_slice().to_vec();

    match address {
        Some(address) => vec![address.as_bytes().to_vec()],
        None => vec![caller_principal],
    }
}

/// Returns the parsed address given the string representation and the expected network.
#[inline]
pub(crate) fn get_address(address: String, network: BitcoinNetwork) -> InscribeResult<Address> {
    Address::from_str(&address)
        .map_err(|_| InscribeError::BadAddress(address.clone()))?
        .require_network(CanisterWallet::map_network(network))
        .map_err(|_| InscribeError::BadAddress(address))
}
