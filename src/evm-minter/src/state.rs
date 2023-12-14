pub use self::config::{Config, ConfigData};

mod config;

#[derive(Debug, Default)]
pub struct State {
    pub config: Config,
}

impl State {
    pub fn init(config: ConfigData) {
        Config::init(config);
    }
}
