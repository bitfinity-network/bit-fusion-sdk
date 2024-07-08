use std::borrow::Cow;
use std::cell::RefCell;

use candid::{Decode, Encode};
use ic_exports::ic_kit::ic;
use ic_log::{init_log, LogSettings, LoggerConfig};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{Bound, CellStructure, StableCell, Storable, VirtualMemory};
use minter_did::error::Error;

use super::memory::{LOG_SETTINGS_MEMORY_ID, MEMORY_MANAGER};

thread_local! {
    static LOG_SETTINGS: RefCell<StableCell<StorableLogSettings, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(LOG_SETTINGS_MEMORY_ID)), StorableLogSettings(LogSettings::default()))
        .expect("failed to initialize log settings cell"));

    static LOGGER_CONFIG: RefCell<Option<LoggerConfig>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone)]
pub struct StorableLogSettings(pub LogSettings);

impl Storable for StorableLogSettings {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::from(Encode!(&self.0).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(Decode!(&bytes, LogSettings).unwrap())
    }
}

/// Handles the runtime logger configuration
#[derive(Default)]
pub struct LoggerConfigService;

impl LoggerConfigService {
    /// Initialize a new LoggerConfigService. Must be called just once
    pub fn init(&mut self, log_settings: LogSettings) -> Result<(), Error> {
        if LOGGER_CONFIG.with(|logger_config| logger_config.borrow().is_some()) {
            return Err(Error::Internal(
                "LoggerConfig already initialized".to_string(),
            ));
        }
        // set settings

        LOG_SETTINGS
            .with(|cell| cell.borrow_mut().set(StorableLogSettings(log_settings)))
            .map_err(|_| Error::Internal("Storage error".to_string()))?;

        // get settings and init log
        let log_settings = LOG_SETTINGS.with(|cell| cell.borrow().get().0.clone());
        let logger_config = init_log(&log_settings)
            .map_err(|e| Error::Internal(format!("Logger init error: {e}")))?;
        LOGGER_CONFIG.with(|config| config.borrow_mut().replace(logger_config));

        // Print this out without using log in case the given parameters prevent logs to be printed.
        ic::print(format!(
            "Initialized logging with settings: {log_settings:?}"
        ));

        Ok(())
    }

    /// Reload the logger configuration
    pub fn reload(&mut self) -> Result<(), Error> {
        if LOGGER_CONFIG.with(|logger_config| logger_config.borrow().is_some()) {
            return Err(Error::Internal(
                "LoggerConfig already initialized".to_string(),
            ));
        }

        // get settings and init log
        let log_settings = LOG_SETTINGS.with(|cell| cell.borrow().get().0.clone());
        let logger_config = init_log(&log_settings)
            .map_err(|e| Error::Internal(format!("Logger init error: {e}")))?;
        LOGGER_CONFIG.with(|config| config.borrow_mut().replace(logger_config));

        Ok(())
    }

    /// Changes the logger filter at runtime
    pub fn set_logger_filter(&mut self, filter: &str) -> Result<(), Error> {
        self.update_log_settings(filter)?;
        LOGGER_CONFIG.with(|config| match *config.borrow_mut() {
            Some(ref logger_config) => {
                logger_config.update_filters(filter);
                Ok(())
            }
            None => Err(Error::Internal("LoggerConfig not initialized".to_string())),
        })
    }

    fn update_log_settings(&mut self, filter: &str) -> Result<(), Error> {
        LOG_SETTINGS
            .with(|cell| {
                let mut cell = cell.borrow_mut();
                let mut log_settings = cell.get().clone();
                log_settings.0.log_filter = Some(filter.to_string());
                cell.set(log_settings)
            })
            .map_err(|_| Error::Internal("Storage error".to_string()))
    }
}
