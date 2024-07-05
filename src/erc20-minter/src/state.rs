use std::fmt;

use bridge_utils::evm_link::EvmLink;
use candid::{CandidType, Principal};
pub use config::Config;
use eth_signer::sign_strategy::{
    ManagementCanisterSigner, SigningKeyId, SigningStrategy, TxSigner,
};
use ic_log::LogSettings;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, VirtualMemory};
use serde::Deserialize;

use self::log::LoggerConfigService;
use crate::memory::{MEMORY_MANAGER, SIGNER_MEMORY_ID};

mod config;
mod log;

type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

pub struct State {
    pub config: Config,
    pub signer: SignerStorage,
    pub logger: LoggerConfigService,
}

impl Default for State {
    fn default() -> Self {
        let default_signer =
            TxSigner::ManagementCanister(ManagementCanisterSigner::new(SigningKeyId::Test, vec![]));
        let signer = SignerStorage::new(
            MEMORY_MANAGER.with(|mm| mm.get(SIGNER_MEMORY_ID)),
            default_signer,
        )
        .expect("failed to initialize transaction signer");

        let logger = LoggerConfigService::default();

        Self {
            config: Default::default(),
            signer,
            logger,
        }
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("config", &self.config)
            .field("scheduler", &"PersistentScheduler")
            .finish()
    }
}

impl State {
    pub fn init(&mut self, admin: Principal, settings: Settings) {
        let signer = settings
            .signing_strategy
            .clone()
            .make_signer(0)
            .expect("failed to make signer according to settings");

        if let Some(log_settings) = &settings.log_settings {
            self.logger.init(log_settings.clone());
        }

        self.config.init(admin, settings);

        self.signer.set(signer).expect("failed to set signer");
    }
}

#[derive(Debug, Clone, Deserialize, CandidType)]
pub struct Settings {
    pub base_evm_link: EvmLink,
    pub wrapped_evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,

    /// Log settings
    #[serde(default)]
    pub log_settings: Option<LogSettings>,
}
