use std::cell::{Cell, RefCell};
use std::rc::Rc;

use candid::Principal;
use did::build::BuildData;
use ic_canister::{
    generate_idl, init, post_upgrade, pre_upgrade, query, update, Canister, Idl, PreUpdate,
};
use ic_exports::ic_cdk::api::call::CallResult;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    BitcoinNetwork, MillisatoshiPerByte, Utxo,
};
use ic_metrics::{Metrics, MetricsStorage};

use crate::build_data::canister_build_data;
use crate::wallet::inscription::Protocol;
use crate::wallet::{self, bitcoin_api};

thread_local! {
    pub(crate) static BITCOIN_NETWORK: Cell<BitcoinNetwork> = Cell::new(BitcoinNetwork::Regtest);

    pub(crate) static ECDSA_DERIVATION_PATH: Vec<Vec<u8>> = vec![];

    pub(crate) static ECDSA_KEY_NAME: RefCell<String> = RefCell::new(String::from(""));
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

        ECDSA_KEY_NAME.with(|key_name| {
            key_name.replace(String::from(match network {
                BitcoinNetwork::Regtest => "dfx_test_key",
                BitcoinNetwork::Mainnet | BitcoinNetwork::Testnet => "test_key_1",
            }))
        });
    }

    /// Returns the balance of the given bitcoin address.
    #[update]
    pub async fn get_balance(&mut self, address: String) -> CallResult<(u64,)> {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        bitcoin_api::get_balance(network, address).await
    }

    /// Returns the UTXOs of the given bitcoin address.
    #[update]
    pub async fn get_utxos(&mut self, address: String) -> Result<Vec<Utxo>, String> {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        bitcoin_api::get_utxos(network, address).await
    }

    /// Returns the 100 fee percentiles measured in millisatoshi/byte.
    /// Percentiles are computed from the last 10,000 transactions (if available).
    #[update]
    pub async fn get_current_fee_percentiles(&mut self) -> CallResult<(Vec<MillisatoshiPerByte>,)> {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        bitcoin_api::get_current_fee_percentiles(network).await
    }

    /// Returns the P2PKH address of this canister at a specific derivation path.
    #[update]
    pub async fn get_p2pkh_address(&mut self) -> String {
        let derivation_path = ECDSA_DERIVATION_PATH.with(|d| d.clone());
        let key_name = ECDSA_KEY_NAME.with(|kn| kn.borrow().to_string());
        let network = BITCOIN_NETWORK.with(|n| n.get());
        wallet::get_p2pkh_address(network, key_name, derivation_path).await
    }

    /// Inscribes and sends the given amount of bitcoin from this canister to the given address.
    /// Returns the commit and reveal transaction IDs.
    #[update]
    pub async fn inscribe(
        &mut self,
        inscription_type: Protocol,
        inscription: String,
        dst_address: Option<String>,
        leftovers_recipient: Option<String>,
    ) -> Result<(String, String), String> {
        let network = BITCOIN_NETWORK.with(|n| n.get());

        match wallet::inscribe(
            network,
            inscription_type,
            inscription,
            dst_address,
            leftovers_recipient,
        )
        .await
        {
            Ok(val) => Ok(val),
            Err(err) => Err(format!("{err}")),
        }
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

    /// Returns the build data of the canister
    #[query]
    pub fn get_canister_build_data(&self) -> BuildData {
        canister_build_data()
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for Inscriber {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}
