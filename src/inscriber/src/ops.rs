use did::{InscribeResult, InscribeTransactions, InscriptionFees};
use ord_rs::MultisigConfig;

use crate::wallet::inscription::{Multisig, Protocol};
use crate::wallet::CanisterWallet;
use crate::Inscriber;

/// Inscribes a message onto the Bitcoin blockchain using the given inscription
/// type.
pub async fn inscribe(
    inscription_type: Protocol,
    inscription: String,
    leftovers_address: String,
    dst_address: Option<String>,
    multisig_config: Option<Multisig>,
    derivation_path: Vec<Vec<u8>>,
) -> InscribeResult<InscribeTransactions> {
    let network = Inscriber::get_network_config();
    let leftovers_address = Inscriber::get_address(leftovers_address, network)?;

    let dst_address = match dst_address {
        None => None,
        Some(dst_address) => Some(Inscriber::get_address(dst_address, network)?),
    };

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

/// Gets the Bitcoin address for the given derivation path.
pub async fn get_bitcoin_address(derivation_path: Vec<Vec<u8>>) -> String {
    let network = Inscriber::get_network_config();

    CanisterWallet::new(derivation_path, network)
        .get_bitcoin_address()
        .await
        .to_string()
}

pub async fn get_inscription_fees(
    inscription_type: Protocol,
    inscription: String,
    multisig_config: Option<Multisig>,
) -> InscribeResult<InscriptionFees> {
    let network = Inscriber::get_network_config();
    let multisig_config = multisig_config.map(|m| MultisigConfig {
        required: m.required,
        total: m.total,
    });

    CanisterWallet::new(vec![], network)
        .get_inscription_fees(inscription_type, inscription, multisig_config)
        .await
}
