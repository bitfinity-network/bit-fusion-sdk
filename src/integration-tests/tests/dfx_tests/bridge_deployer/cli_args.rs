use std::path::PathBuf;
use std::process::Command;

use candid::Principal;
use eth_signer::{Signer as _, Wallet};

use crate::context::{CanisterType, TestContext as _};
use crate::dfx_tests::DfxTestContext;

/// The name of the user with a thick wallet.
pub const ADMIN: &str = "max";
/// A private key for testing purposes.
const HARDHAT_ETH_PRIVATE_KEY: &str =
    "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

pub struct CommonCliArgs {
    pub evm: String,
    pub private_key: String,
    pub identity_path: PathBuf,
}

impl CommonCliArgs {
    pub async fn new(ctx: &DfxTestContext) -> Self {
        let private_key_bytes = hex::decode(HARDHAT_ETH_PRIVATE_KEY).expect("Invalid private key");
        let wallet = Wallet::from_bytes(&private_key_bytes).expect("Invalid private key");

        let client = ctx.evm_client(ctx.admin_name());
        client
            .admin_mint_native_tokens(wallet.address().into(), u128::MAX.into())
            .await
            .expect("failed to mint native tokens (called failed)")
            .expect("failed to mint native tokens (call error)");

        let evm_principal = ctx.canisters.evm().to_text();

        let mut identity_path = dirs::home_dir().expect("failed to get home dir");
        identity_path.push(".config");
        identity_path.push("dfx/identity");
        identity_path.push(ADMIN);
        identity_path.push("identity.pem");

        Self {
            evm: evm_principal,
            private_key: HARDHAT_ETH_PRIVATE_KEY.to_string(),
            identity_path,
        }
    }
}

pub struct DeployCliArgs {
    pub brc20_bridge: PathBuf,
    pub btc_bridge: PathBuf,
    pub erc20_bridge: PathBuf,
    pub icrc2_bridge: PathBuf,
    pub rune_bridge: PathBuf,
    pub wallet_canister: String,
}

impl DeployCliArgs {
    pub async fn new(ctx: &DfxTestContext) -> Self {
        let brc20_bridge_wasm_path = ctx.get_wasm_path(CanisterType::Brc20Bridge).await;
        let btc_bridge_wasm_path = ctx.get_wasm_path(CanisterType::BtcBridge).await;
        let icrc2_bridge_wasm_path = ctx.get_wasm_path(CanisterType::Icrc2Bridge).await;
        let rune_bridge_wasm_path = ctx.get_wasm_path(CanisterType::RuneBridge).await;
        let erc_20_bridge_wasm_path = ctx.get_wasm_path(CanisterType::Erc20Bridge).await;

        let wallet_canister = Self::wallet_canister().to_text();

        Self {
            brc20_bridge: brc20_bridge_wasm_path,
            btc_bridge: btc_bridge_wasm_path,
            erc20_bridge: erc_20_bridge_wasm_path,
            icrc2_bridge: icrc2_bridge_wasm_path,
            rune_bridge: rune_bridge_wasm_path,
            wallet_canister,
        }
    }

    /// Get the wallet canister principal
    fn wallet_canister() -> Principal {
        // set the identity to max
        let rc = Command::new("dfx")
            .args(["identity", "use", ADMIN])
            .status()
            .expect("failed to set identity to max");

        assert!(rc.success(), "failed to set identity to max");

        // get the wallet canister id
        let wallet_canister = Command::new("dfx")
            .args(["identity", "get-wallet"])
            .output()
            .expect("failed to get wallet canister id")
            .stdout
            .iter()
            .map(|&b| b as char)
            .collect::<String>()
            .trim()
            .to_string();

        Principal::from_text(&wallet_canister).expect("Invalid principal")
    }
}
