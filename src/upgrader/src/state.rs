use candid::Principal;
use config::Config;
use eth_signer::sign_strategy::SigningStrategy;
use ic_stable_structures::default_ic_memory_manager;
use signer::SignerInfo;

mod config;
pub mod signer;

/// State of a minter canister.
#[derive(Default)]
pub struct State {
    /// Minter canister configuration.
    pub config: Config,

    /// Transaction signing info.
    pub signer: SignerInfo,
}

impl State {
    /// Clear the state and set initial data from settings.
    pub fn reset(&mut self, settings: Settings) {
        self.signer
            .reset(settings.signing_strategy.clone(), 0)
            .expect("failed to set signer");
        self.config.reset(settings);
    }
}

/// State settings.
#[derive(Debug, Clone)]
pub struct Settings {
    pub owner: Principal,
    pub signing_strategy: SigningStrategy,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            owner: Principal::management_canister(),
            signing_strategy: SigningStrategy::Local {
                private_key: [218u8; 32],
            },
        }
    }
}
