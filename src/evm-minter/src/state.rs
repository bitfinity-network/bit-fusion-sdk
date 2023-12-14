pub use self::config::{Config, ConfigData};

mod config;

#[derive(Debug)]
pub struct State {
    pub config: Config,
}

impl Default for State {
    fn default() -> Self {
        Self { config: Default::default() }
    }
}

impl State {
    pub fn init(config: ConfigData) {
        Config::init(config);
    }
}
