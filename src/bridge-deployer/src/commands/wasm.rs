use std::path::Path;

use super::Bridge;

const BRC20_BRIDGE_DEFAULT_PATH: &str = ".artifact/brc20-bridge.wasm.gz";
const BTC_BRIDGE_DEFAULT_PATH: &str = ".artifact/btc-bridge.wasm.gz";
const ERC20_BRIDGE_DEFAULT_PATH: &str = ".artifact/erc20-bridge.wasm.gz";
const ICRC2_BRIDGE_DEFAULT_PATH: &str = ".artifact/icrc2-bridge.wasm.gz";
const RUNE_BRIDGE_DEFAULT_PATH: &str = ".artifact/rune-bridge.wasm.gz";

/// Get the default wasm path based on the [`Bridge`] type.
///
/// Returns a [`Path`] to the default wasm file.
pub fn get_default_wasm_path(bridge: &Bridge) -> &Path {
    Path::new(match bridge {
        Bridge::Brc20 { .. } => BRC20_BRIDGE_DEFAULT_PATH,
        Bridge::Btc { .. } => BTC_BRIDGE_DEFAULT_PATH,
        Bridge::Erc20 { .. } => ERC20_BRIDGE_DEFAULT_PATH,
        Bridge::Icrc { .. } => ICRC2_BRIDGE_DEFAULT_PATH,
        Bridge::Rune { .. } => RUNE_BRIDGE_DEFAULT_PATH,
    })
}
