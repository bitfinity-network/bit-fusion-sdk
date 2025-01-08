use std::borrow::Cow;

use super::Bridge;

const BRC20_BRIDGE_WASM: &[u8] = include_bytes!("../../../../.artifact/brc20-bridge.wasm");
const BTC_BRIDGE_WASM: &[u8] = include_bytes!("../../../../.artifact/btc-bridge.wasm");
const ERC20_BRIDGE_WASM: &[u8] = include_bytes!("../../../../.artifact/erc20-bridge.wasm");
const ICRC2_BRIDGE_WASM: &[u8] = include_bytes!("../../../../.artifact/icrc2-bridge.wasm");
const RUNE_BRIDGE_WASM: &[u8] = include_bytes!("../../../../.artifact/rune-bridge.wasm");

/// Get wasm bytes for the specified [`Bridge`] type.
///
/// Returns a borrowed slice as [`Cow`] of the wasm bytes.
pub fn get_wasm(bridge: &Bridge) -> Cow<'static, [u8]> {
    Cow::Borrowed(match bridge {
        Bridge::Brc20 { .. } => BRC20_BRIDGE_WASM,
        Bridge::Btc { .. } => BTC_BRIDGE_WASM,
        Bridge::Erc20 { .. } => ERC20_BRIDGE_WASM,
        Bridge::Icrc { .. } => ICRC2_BRIDGE_WASM,
        Bridge::Rune { .. } => RUNE_BRIDGE_WASM,
    })
}
