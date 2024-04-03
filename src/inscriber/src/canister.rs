use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::Address;
use candid::Principal;
use did::{BuildData, InscribeError, InscribeResult, InscribeTransactions, InscriptionFees};
use ic_canister::{
    generate_idl, init, post_upgrade, pre_upgrade, query, update, Canister, Idl, PreUpdate,
};
use ethers_core::abi::ethereum_types::H520;
use ethers_core::types::{Signature, H160};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, GetUtxosResponse};
use ic_metrics::{Metrics, MetricsStorage};
use serde_bytes::ByteBuf;

use crate::build_data::canister_build_data;
use crate::constant::SUPPORTED_ENDPOINTS;
use crate::http::{HttpRequest, HttpResponse, Rpc};
use crate::wallet::inscription::{Multisig, Protocol};
use crate::wallet::{bitcoin_api, CanisterWallet};
use crate::{http_response, ops};

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
        let derivation_path = Self::derivation_path(None);
        ops::get_bitcoin_address(derivation_path).await
    }

        /// Returns the estimated inscription fees for the given inscription.
    #[update]
    pub async fn get_inscription_fees(
        &self,
        inscription_type: Protocol,
        inscription: String,
        multisig_config: Option<Multisig>,
    ) -> InscribeResult<InscriptionFees> {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        let multisig_config = multisig_config.map(|m| MultisigConfig {
            required: m.required,
            total: m.total,
        });

        CanisterWallet::new(vec![], network)
            .get_inscription_fees(inscription_type, inscription, multisig_config)
            .await
    }


    /// Returns the estimated inscription fees for the given inscription.
    #[update]
    pub async fn get_inscription_fees(
        &self,
        inscription_type: Protocol,
        inscription: String,
        multisig_config: Option<Multisig>,
    ) -> InscribeResult<InscriptionFees> {
        let network = BITCOIN_NETWORK.with(|n| n.get());
        let multisig_config = multisig_config.map(|m| MultisigConfig {
            required: m.required,
            total: m.total,
        });

        CanisterWallet::new(vec![], network)
            .get_inscription_fees(inscription_type, inscription, multisig_config)
            .await
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
        let derivation_path = Self::derivation_path(None);
        ops::inscribe(
            inscription_type,
            inscription,
            leftovers_address,
            dst_address,
            multisig_config,
            derivation_path,
        )
        .await
    }

    #[query]
    pub async fn http_request(&mut self, req: HttpRequest) -> HttpResponse {
        if req.method.as_ref() != "POST"
            || req.headers.get("content-type").map(|s| s.as_ref()) != Some("application/json")
        {
            return HttpResponse {
                status_code: 400,
                headers: HashMap::new(),
                body: ByteBuf::from("Bad Request: only supports JSON-RPC.".as_bytes()),
                upgrade: None,
            };
        }

        if !SUPPORTED_ENDPOINTS.contains(&req.url.as_str()) {
            return HttpResponse::error(400, "endpoint not supported".to_owned());
        }

        HttpResponse::upgrade_response()
    }

    #[update]
    pub async fn http_request_update(&self, req: HttpRequest) -> HttpResponse {
        let response = match req.decode_body() {
            Ok(res) => res,
            Err(err) => return *err,
        };

        let response = Rpc::process_request(response, &Rpc::handle_calls).await;

        let response = http_response!(serde_json::to_vec(&response));

        HttpResponse {
            status_code: 200,
            headers: HashMap::from([("content-type".into(), "application/json".into())]),
            body: ByteBuf::from(&*response),
            upgrade: None,
        }
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
    /// Returns the derivation path to use for signing/verifying based on the caller principal or provided address.
    pub(crate) fn derivation_path(address: Option<H160>) -> Vec<Vec<u8>> {
        let caller_principal = ic_exports::ic_cdk::caller().as_slice().to_vec();

        match address {
            Some(address) => vec![address.as_bytes().to_vec()],
            None => vec![caller_principal],
        }
    }

    #[inline]
    /// Returns the parsed address given the string representation and the expected network.
    pub(crate) fn get_address(address: String, network: BitcoinNetwork) -> InscribeResult<Address> {
        Address::from_str(&address)
            .map_err(|_| InscribeError::BadAddress(address.clone()))?
            .require_network(CanisterWallet::map_network(network))
            .map_err(|_| InscribeError::BadAddress(address))
    }

    /// Recovers the public key from the given message and signature
    pub fn recover_pubkey(message: String, signature: H520) -> did::InscribeResult<H160> {
        let signature = Signature::try_from(signature.as_bytes())?;
        let address = signature.recover(message)?;

        Ok(address)
    }
}

impl Metrics for Inscriber {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}
