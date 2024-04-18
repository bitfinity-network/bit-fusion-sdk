use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::Address;
use candid::Principal;
use ethers_core::types::H160;
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, GetUtxosResponse};
use ic_metrics::{Metrics, MetricsStorage};
use serde_bytes::ByteBuf;

use crate::build_data::canister_build_data;
use crate::constant::SUPPORTED_ENDPOINTS;
use crate::http::{HttpRequest, HttpResponse, Rpc};
use crate::interface::{
    bitcoin_api, Brc20TransferTransactions, BuildData, InscribeError, InscribeResult,
    InscribeTransactions, InscriptionFees, Multisig, Protocol,
};
use crate::state::{InscriberConfig, State, BITCOIN_NETWORK, INSCRIBER_STATE};
use crate::wallet::CanisterWallet;
use crate::{http_response, ops};

#[derive(Canister, Clone, Debug)]
pub struct Inscriber {
    #[id]
    id: Principal,
}

impl PreUpdate for Inscriber {}

impl Inscriber {
    #[init]
    pub fn init(&mut self, config: InscriberConfig) {
        Self::get_inscriber_state().borrow_mut().configure(config);
    }

    /// Returns the balance of the given bitcoin address.
    #[update]
    pub async fn get_balance(&mut self, address: String) -> u64 {
        let network = Self::get_network_config();
        bitcoin_api::get_balance(network, address).await
    }

    /// Returns the UTXOs of the given bitcoin address.
    #[update]
    pub async fn get_utxos(&mut self, address: String) -> GetUtxosResponse {
        let network = Self::get_network_config();
        bitcoin_api::get_utxos(network, address)
            .await
            .expect("Failed to retrieve UTXOs")
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
        ops::get_inscription_fees(inscription_type, inscription, multisig_config).await
    }

    /// Inscribes and sends the inscribed sat from this canister to the given address.
    /// Returns the commit and reveal transaction IDs.
    #[update]
    pub async fn inscribe(
        &mut self,
        inscription_type: Protocol,
        inscription: String,
        leftovers_address: String,
        dst_address: String,
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

    /// Inscribes and sends the inscribed sat from this canister to the given address.
    #[update]
    pub async fn brc20_transfer(
        &mut self,
        inscription: String,
        leftovers_address: String,
        dst_address: String,
        multisig_config: Option<Multisig>,
    ) -> InscribeResult<Brc20TransferTransactions> {
        let derivation_path = Self::derivation_path(None);
        ops::brc20_transfer(
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

        let request = match req.decode_body() {
            Ok(res) => res,
            Err(err) => return *err,
        };

        if !SUPPORTED_ENDPOINTS.contains(&request.method.as_str()) {
            return HttpResponse::error(400, "endpoint not supported".to_owned());
        }

        HttpResponse::upgrade_response()
    }

    #[update]
    pub async fn http_request_update(&self, req: HttpRequest) -> HttpResponse {
        let request = match req.decode_body() {
            Ok(res) => res,
            Err(err) => return *err,
        };

        let response = Rpc::process_request(request, &Rpc::handle_calls).await;

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

    pub fn idl() -> Idl {
        generate_idl!()
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

    #[inline]
    pub fn get_network_config() -> BitcoinNetwork {
        BITCOIN_NETWORK.with(|n| n.get())
    }

    pub fn get_inscriber_state() -> Rc<RefCell<State>> {
        INSCRIBER_STATE.with(|state| state.clone())
    }
}

impl Metrics for Inscriber {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}
