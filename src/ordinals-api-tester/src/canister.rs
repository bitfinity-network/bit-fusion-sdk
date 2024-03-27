use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use candid::{CandidType, Principal};
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk::api::management_canister::http_request::HttpResponse;
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use ordinals_api::{brc20, http, inscription};
use serde::Deserialize;

#[derive(Debug, Default, IcStorage, CandidType, Deserialize)]
struct State {
    pub base_api_url: String,
    pub http_mocks: HashMap<String, HttpResponse>,
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self {
            base_api_url: String::from(""),
            http_mocks: HashMap::new(),
        }
    }
}

#[derive(Canister, Clone, Debug)]
pub struct OrdinalsApiTester {
    #[id]
    id: Principal,

    #[state]
    state: Rc<RefCell<State>>,
}

impl PreUpdate for OrdinalsApiTester {}

impl OrdinalsApiTester {
    #[init]
    pub fn init(&mut self, base_api_url: String) {
        self.state.borrow_mut().base_api_url = base_api_url;
    }

    #[update]
    pub fn set_http_mock(&mut self, url: String, resp: HttpResponse) {
        self.state.borrow_mut().http_mocks.insert(url, resp);
    }

    #[update]
    pub fn get_http_mock(&self, url: String) -> Option<HttpResponse> {
        self.state.borrow().http_mocks.get(&url).cloned()
    }

    #[update]
    pub async fn get_brc20_tokens(
        &self,
        offset: u64,
        limit: u64,
    ) -> Option<http::PaginatedResp<brc20::state::Brc20Token>> {
        brc20::api::get_brc20_tokens(&self.get_base_api_url(), offset, limit)
            .await
            .expect("Can't perform HTTP outcall")
    }

    #[update]
    pub async fn get_brc20_token_by_ticker(
        &self,
        ticker: String,
    ) -> Option<brc20::state::Brc20TokenDetails> {
        brc20::api::get_brc20_token_by_ticker(&self.get_base_api_url(), &ticker)
            .await
            .expect("Can't perform HTTP outcall")
    }

    #[update]
    pub async fn get_brc20_token_holders_by_ticker(
        &self,
        ticker: String,
        offset: u64,
        limit: u64,
    ) -> Option<http::PaginatedResp<brc20::state::Brc20Holder>> {
        brc20::api::get_brc20_token_holders_by_ticker(
            &self.get_base_api_url(),
            &ticker,
            offset,
            limit,
        )
        .await
        .expect("Can't perform HTTP outcall")
    }

    #[update]
    pub async fn get_brc20_token_balance_by_address(
        &self,
        address: String,
        ticker: String,
    ) -> Option<http::PaginatedResp<brc20::state::Brc20Balance>> {
        brc20::api::get_brc20_token_balance_by_address(&self.get_base_api_url(), &address, &ticker)
            .await
            .expect("Can't perform HTTP outcall")
    }

    #[update]
    pub async fn get_inscription_by_id(
        &self,
        id: String,
    ) -> Option<inscription::state::Inscription> {
        inscription::api::get_inscription_by_id(&self.get_base_api_url(), &id)
            .await
            .expect("Can't perform HTTP outcall")
    }

    #[update]
    pub async fn get_inscription_transfers_by_id(
        &self,
        id: String,
        offset: u64,
        limit: u64,
    ) -> Option<http::PaginatedResp<inscription::state::InscriptionLocation>> {
        inscription::api::get_inscription_transfers_by_id(
            &self.get_base_api_url(),
            &id,
            offset,
            limit,
        )
        .await
        .expect("Can't perform HTTP outcall")
    }

    #[query]
    pub fn get_base_api_url(&self) -> String {
        self.state.borrow().base_api_url.clone()
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for OrdinalsApiTester {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        MetricsStorage::get()
    }
}
