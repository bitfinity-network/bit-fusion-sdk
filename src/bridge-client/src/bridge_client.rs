use bridge_did::error::BTFResult;
use bridge_did::id256::Id256;
use bridge_did::order::SignedMintOrder;
use candid::Principal;
use did::H160;
use did::build::BuildData;
use ic_canister_client::{CanisterClient, CanisterClientResult};
use ic_log::did::{LogCanisterError, LogCanisterSettings, LoggerPermission, Pagination};
use ic_log::writer::Logs;

#[async_trait::async_trait]
pub trait BridgeCanisterClient<C: CanisterClient> {
    fn client(&self) -> &C;

    /// Updates the runtime configuration of the logger with a new filter in the same form as the `RUST_LOG`
    /// environment variable.
    ///
    /// Example of valid filters:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    ///
    /// This method is only for canister owner.
    async fn set_logger_filter(
        &self,
        filter: String,
    ) -> CanisterClientResult<Result<(), LogCanisterError>> {
        self.client().update("set_logger_filter", (filter,)).await
    }

    /// Gets the logs
    ///
    /// # Arguments
    /// - `count` is the number of logs to return
    ///
    /// This method is only for canister owner.
    async fn ic_logs(&self, pagination: Pagination) -> CanisterClientResult<Logs> {
        self.client().query("ic_logs", (pagination,)).await
    }

    async fn set_logger_in_memory_records(&self, max_log_count: usize) -> CanisterClientResult<()> {
        self.client()
            .update("set_logger_in_memory_records", (max_log_count,))
            .await
    }

    async fn get_logger_settings(&self) -> CanisterClientResult<LogCanisterSettings> {
        self.client().query("get_logger_settings", ()).await
    }

    async fn add_logger_permission(
        &self,
        to: Principal,
        permission: LoggerPermission,
    ) -> CanisterClientResult<()> {
        self.client()
            .update("add_logger_permission", (to, permission))
            .await
    }
    async fn remove_logger_permission(
        &self,
        from: Principal,
        permission: LoggerPermission,
    ) -> CanisterClientResult<()> {
        self.client()
            .update("remove_logger_permission", (from, permission))
            .await
    }

    /// Returns principal of canister owner.
    async fn get_owner(&self) -> CanisterClientResult<Principal> {
        self.client().query("get_owner", ()).await
    }

    /// Sets a new principal for canister owner.
    ///
    /// This method should be called only by current owner,
    /// else `Error::NotAuthorised` will be returned.
    async fn set_owner(&mut self, owner: Principal) -> CanisterClientResult<()> {
        self.client().update("set_owner", (owner,)).await
    }

    /// Returns principal of EVM canister with which the bridge canister works.
    async fn get_bridge_canister_evm_address(&self) -> CanisterClientResult<BTFResult<H160>> {
        self.client()
            .update("get_bridge_canister_evm_address", ())
            .await
    }

    /// Returns principal of EVM canister with which the bridge canister works.
    async fn get_evm_principal(&self) -> CanisterClientResult<Principal> {
        self.client().query("get_evm_principal", ()).await
    }

    /// Sets btf bridge contract address.
    async fn set_btf_bridge_contract(&self, address: &H160) -> CanisterClientResult<()> {
        self.client()
            .update("set_btf_bridge_contract", (address,))
            .await
    }

    /// Returns the address of the BTF bridge contract in EVM canister.
    async fn get_btf_bridge_contract(&self) -> CanisterClientResult<BTFResult<Option<H160>>> {
        self.client().update("get_btf_bridge_contract", ()).await
    }

    /// Returns `(nonce, mint_order)` pairs for the given sender id.
    async fn list_mint_orders(
        &self,
        sender: Id256,
        src_token: Id256,
    ) -> CanisterClientResult<Vec<(u32, SignedMintOrder)>> {
        self.client()
            .query("list_mint_orders", (sender, src_token))
            .await
    }

    /// Returns the build data of the canister.
    async fn get_canister_build_data(&self) -> CanisterClientResult<BuildData> {
        self.client().query("get_canister_build_data", ()).await
    }

    /// Adds the given principal to the whitelist.
    async fn add_to_whitelist(&self, principal: Principal) -> CanisterClientResult<BTFResult<()>> {
        self.client().update("add_to_whitelist", (principal,)).await
    }

    /// Removes the given principal from the whitelist.
    async fn remove_from_whitelist(
        &self,
        principal: Principal,
    ) -> CanisterClientResult<BTFResult<()>> {
        self.client()
            .update("remove_from_whitelist", (principal,))
            .await
    }
}

pub struct GenericBridgeClient<C> {
    client: C,
}

impl<C: CanisterClient> GenericBridgeClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C: CanisterClient> BridgeCanisterClient<C> for GenericBridgeClient<C> {
    fn client(&self) -> &C {
        &self.client
    }
}
