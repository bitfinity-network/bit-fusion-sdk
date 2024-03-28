use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::Address;
use candid::Principal;
use did::{BuildData, InscribeError, InscribeResult, InscribeTransactions};
use ic_canister::{
    generate_idl, init, post_upgrade, pre_upgrade, query, update, Canister, Idl, PreUpdate,
};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, GetUtxosResponse};
use ic_metrics::{Metrics, MetricsStorage};
use ord_rs::MultisigConfig;

use crate::build_data::canister_build_data;
use crate::wallet::inscription::{Multisig, Protocol};
use crate::wallet::{bitcoin_api, CanisterWallet};

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
        let derivation_path = Self::derivation_path();
        let network = BITCOIN_NETWORK.with(|n| n.get());

        CanisterWallet::new(derivation_path, network)
            .get_bitcoin_address()
            .await
            .to_string()
    }

    /// Inscribes and sends the inscribed sat from this canister to the given address.
    /// Returns the commit and reveal transaction IDs.
    #[update]
    pub async fn inscribe(
        &mut self,
        inscription_type: Protocol,
        inscription: String,
        leftovers_address: String,
        dst_address: Option<String>,
        multisig_config: Option<Multisig>,
    ) -> InscribeResult<InscribeTransactions> {
        let derivation_path = Self::derivation_path();
        let network = BITCOIN_NETWORK.with(|n| n.get());
        let leftovers_address = Self::get_address(leftovers_address, network)?;

        let dst_address = match dst_address {
            None => None,
            Some(dst_address) => Some(Self::get_address(dst_address, network)?),
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

    #[inline]
    fn derivation_path() -> Vec<Vec<u8>> {
        let caller_principal = ic_exports::ic_cdk::caller().as_slice().to_vec();

        vec![caller_principal] // Derivation path
    }

    #[inline]
    /// Returns the parsed address given the string representation and the expected network.
    fn get_address(address: String, network: BitcoinNetwork) -> InscribeResult<Address> {
        Address::from_str(&address)
            .map_err(|_| InscribeError::BadAddress(address.clone()))?
            .require_network(CanisterWallet::map_network(network))
            .map_err(|_| InscribeError::BadAddress(address))
    }
}

impl Metrics for Inscriber {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}
