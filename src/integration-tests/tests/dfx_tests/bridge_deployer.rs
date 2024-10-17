mod cli_args;
mod eval;

use std::path::{Path, PathBuf};
use std::process::Command;

use cli_args::{CommonCliArgs, DeployCliArgs};
use tempfile::TempDir;

use super::DfxTestContext;
use crate::context::{CanisterType, TestContext};
use crate::vars_by_file;

async fn setup() -> DfxTestContext {
    restore_manifest_dir();
    DfxTestContext::new(&CanisterType::BRIDGE_DEPLOYER_TEST_SET).await
}

macro_rules! test_deploy {
    ($bridge_name:ident, $test_name:ident, $trycmd_file:expr) => {
        #[tokio::test]
        #[serial_test::serial]
        #[cfg(feature = "dfx_tests")]
        async fn $test_name() {
            let ctx = setup().await;

            let CommonCliArgs {
                evm: evm_principal,
                private_key,
                identity_path,
            } = CommonCliArgs::new(&ctx).await;

            let DeployCliArgs {
                $bridge_name: bridge_name,
                wallet_canister,
                ..
            } = DeployCliArgs::new(&ctx).await;

            let ckbtc_ledger = ctx.canisters.icrc1_ledger().to_text();
            let ckbtc_minter = ctx.canisters.ck_btc_minter().to_text();

            let admin_principal = ctx.admin().to_text();

            // get the output dir for evaluated trycmd files
            let trycmd_output_dir = TempDir::new().expect("failed to create temp file");

            // eval the trycmd files
            eval::eval_trycmd(
                [
                    ("IDENTITY_PATH", identity_path.display().to_string()),
                    ("PRIVATE_KEY", private_key.to_string()),
                    ("ADMIN_PRINCIPAL", admin_principal.to_string()),
                    ("WALLET_CANISTER", wallet_canister.to_string()),
                    ("EVM_PRINCIPAL", evm_principal.to_string()),
                    ("CKBTC_LEDGER", ckbtc_ledger.to_string()),
                    ("CKBTC_MINTER", ckbtc_minter.to_string()),
                ],
                &vars_by_file! {
                    $trycmd_file => { "WASM_PATH" => bridge_name.display() },
                },
                Path::new("./tests/bridge_deployer/deploy"),
                trycmd_output_dir.path(),
                $trycmd_file,
            )
            .expect("failed to eval trycmd files");

            // change cwd to workspace root
            init_workspace().expect("failed to get workspace root");

            let case = format!("{}/*.eval.trycmd", trycmd_output_dir.path().display());
            trycmd::TestCases::new().case(&case).run();

            // restore the manifest dir
            // otherwise other tests may start in the wrong directory
            restore_manifest_dir();

            trycmd_output_dir.close().expect("failed to close temp dir");
        }
    };
}

test_deploy!(
    brc20_bridge,
    test_should_deploy_brc20_bridge,
    "brc20_bridge.trycmd"
);
test_deploy!(
    btc_bridge,
    test_should_deploy_btc_bridge,
    "btc_bridge.trycmd"
);
test_deploy!(
    erc20_bridge,
    test_should_deploy_erc20_bridge,
    "erc20_bridge.trycmd"
);
test_deploy!(
    icrc2_bridge,
    test_should_deploy_icrc2_bridge,
    "icrc2_bridge.trycmd"
);
test_deploy!(
    rune_bridge,
    test_should_deploy_rune_bridge,
    "rune_bridge.trycmd"
);

/// Change the current working directory to the workspace root
/// And clean the openzeppelin folder
fn init_workspace() -> anyhow::Result<PathBuf> {
    let cmd_output = Command::new("cargo")
        .args(["metadata", "--format-version=1"])
        .output()?;

    if !cmd_output.status.success() {
        anyhow::bail!("Failed to get workspace root");
    }

    let json =
        serde_json::from_str::<serde_json::Value>(String::from_utf8(cmd_output.stdout)?.as_str())?;

    let path = json
        .get("workspace_root")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get workspace root"))?;

    std::env::set_current_dir(&path).expect("failed to change cwd");

    // delete `.openzeppelin`; this is necessary because the `openzeppelin` folder
    // causes some complaints when running the tests with a 'contract not found error'
    let _ = std::fs::remove_dir_all("./solidity/.openzeppelin");

    Ok(PathBuf::from(path))
}

/// Restore the manifest directory as the current working directory
fn restore_manifest_dir() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    std::env::set_current_dir(&manifest_dir).expect("failed to change cwd");
}
