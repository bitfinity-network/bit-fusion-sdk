use std::cell::{Cell, RefCell};
use std::rc::Rc;

use candid::Principal;
use did::build::BuildData;
use ic_canister::{
    generate_idl, init, post_upgrade, pre_upgrade, query, update, Canister, Idl, PreUpdate,
};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, GetUtxosResponse};
use ic_metrics::{Metrics, MetricsStorage};
use ord_rs::MultisigConfig;

use crate::build_data::canister_build_data;
use crate::wallet::inscription::{Multisig, Protocol};
use crate::wallet::{self, bitcoin_api};

thread_local! {
    pub(crate) static BITCOIN_NETWORK: Cell<BitcoinNetwork> = const { Cell::new(BitcoinNetwork::Regtest) };
}

#[derive(Canister, Clone, Debug)]
pub struct Inscriber {
    #[id]
    id: Principal,
}

impl PreUpdate for Inscriber {}

impl Inscriber {
    #[init]
    pub fn init(&mut self, network: BitcoinNetwork) {
        crate::register_custom_getrandom();
        BITCOIN_NETWORK.with(|n| n.set(network));
    }

    /// Returns the balance of the given bitcoin address.
    #[update]
    pub async fn get_balance(&mut self, address: String) -> u64 {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        bitcoin_api::get_balance(network, address).await
    }

    /// Returns the UTXOs of the given bitcoin address.
    #[update]
    pub async fn get_utxos(&mut self, address: String) -> GetUtxosResponse {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        bitcoin_api::get_utxos(network, address).await.unwrap()
    }

    /// Returns bech32 bitcoin `Address` of this canister at the given derivation path.
    #[update]
    pub async fn get_bitcoin_address(&mut self) -> String {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        wallet::get_bitcoin_address(network).await.to_string()
    }

    /// Inscribes and sends the given amount of bitcoin from this canister to the given address.
    /// Returns the commit and reveal transaction IDs.
    #[update]
    pub async fn inscribe(
        &mut self,
        inscription_type: Protocol,
        inscription: String,
        dst_address: Option<String>,
        multisig_config: Option<Multisig>,
    ) -> (String, String) {
        let network = BITCOIN_NETWORK.with(|n| n.get());

        let multisig_config = multisig_config.map(|m| MultisigConfig {
            required: m.required,
            total: m.total,
        });

        wallet::inscribe(
            network,
            inscription_type,
            inscription,
            dst_address,
            multisig_config,
        )
        .await
        .unwrap()
    }

    /// Returns the build data of the canister
    #[query]
    pub fn get_canister_build_data(&self) -> BuildData {
        canister_build_data()
    }

    #[pre_upgrade]
    fn pre_upgrade(&self) {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        ic_exports::ic_cdk::storage::stable_save((network,))
            .expect("Failed to save network to stable memory");
    }

    #[post_upgrade]
    fn post_upgrade(&mut self) {
        let network = ic_exports::ic_cdk::storage::stable_restore::<(BitcoinNetwork,)>()
            .expect("Failed to read network from stable memory.")
            .0;
        self.init(network);
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }

    #[update]
    pub async fn test_ecdsa_api_signing(&mut self) {
        use ord_rs::ExternalSigner;
        use wallet::EcdsaSigner;

        let signer = EcdsaSigner;

        let ecdsa_pubkey = signer.ecdsa_public_key().await;
        let message = String::from("Hello world!");
        let signature = signer.sign_with_ecdsa(message.clone()).await;

        ic_exports::ic_cdk::print("Verifying ECDSA signature signed with ECDSA API");
        assert!(signer.verify_ecdsa(signature, message, ecdsa_pubkey).await);
        ic_exports::ic_cdk::print("ECDSA signature verified");
    }

    #[update]
    pub async fn test_private_key_ecdsa_signing(&mut self) {
        // WIF: cQ2WiaKM1RnsytieLDqN6sqBw3wDSkgCHQdgfGUEC5qiYaP8sDaN
        // Mnemonic: salon embody gorilla simple half olympic portion miss blossom mammal involve lunch
        // Address: bcrt1q4td8andehe8wft7p9xkl8x3a9t4ax2y59lfy8l
        use bitcoin::key::Secp256k1;
        use bitcoin::secp256k1::hashes::{sha256, Hash};
        use bitcoin::secp256k1::Message;
        use bitcoin::PrivateKey;

        let secp = Secp256k1::new();

        let private_key =
            PrivateKey::from_wif("cQ2WiaKM1RnsytieLDqN6sqBw3wDSkgCHQdgfGUEC5qiYaP8sDaN").unwrap();
        let public_key = private_key.public_key(&secp);

        let msg_hash = sha256::Hash::hash("Hello world!".as_bytes())
            .as_byte_array()
            .to_vec();

        let message = Message::from_digest_slice(&msg_hash).unwrap();
        let signature = secp.sign_ecdsa(&message, &private_key.inner);

        ic_exports::ic_cdk::print("Verifying ECDSA signature signed with private key");
        assert!(secp
            .verify_ecdsa(&message, &signature, &public_key.inner)
            .is_ok());
        ic_exports::ic_cdk::print("ECDSA signature verified");
    }
}

impl Metrics for Inscriber {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}
