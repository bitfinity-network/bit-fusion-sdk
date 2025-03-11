mod cli_args;
mod eval;

use std::path::{Path, PathBuf};
use std::process::Command;

use bridge_client::BridgeCanisterClient as _;
use cli_args::{CommonCliArgs, DeployCliArgs, HARDHAT_ETH_PRIVATE_KEY};
use eth_signer::Wallet;
use ethers_core::types::H160;
use tempfile::TempDir;

use super::{DfxTestContext, ADMIN};
use crate::context::{CanisterType, TestContext};

async fn setup(canister_set: &[CanisterType]) -> DfxTestContext {
    restore_manifest_dir();
    DfxTestContext::new(canister_set).await
}

macro_rules! test_deploy {
    ($test_name:ident, $trycmd_file:expr, $trycmd_path:expr) => {
        #[tokio::test]
        #[serial_test::serial]
        #[cfg(feature = "dfx_tests")]
        async fn $test_name() {
            let ctx = setup(&[
                CanisterType::Evm,
                CanisterType::Signature,
                CanisterType::Kyt,
                CanisterType::CkBtcLedger,
                CanisterType::CkBtcMinter,
            ])
            .await;

            let CommonCliArgs {
                evm: evm_principal,
                evm_rpc,
                evm_node: _evm_node,
                private_key,
                identity_path,
            } = CommonCliArgs::new(&ctx).await;

            let DeployCliArgs {
                brc20_bridge,
                btc_bridge,
                erc20_bridge,
                icrc2_bridge,
                rune_bridge,
                wallet_canister,
                ..
            } = DeployCliArgs::new(&ctx).await;

            let ckbtc_ledger = ctx.canisters.ckbtc_ledger().to_text();
            let ckbtc_minter = ctx.canisters.ckbtc_minter().to_text();

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
                    ("EVM_RPC", evm_rpc.to_string()),
                    ("CKBTC_LEDGER", ckbtc_ledger.to_string()),
                    ("CKBTC_MINTER", ckbtc_minter.to_string()),
                    ("BRC20_BRIDGE_WASM_PATH", brc20_bridge.display().to_string()),
                    ("BTC_BRIDGE_WASM_PATH", btc_bridge.display().to_string()),
                    ("ERC20_BRIDGE_WASM_PATH", erc20_bridge.display().to_string()),
                    ("ICRC2_BRIDGE_WASM_PATH", icrc2_bridge.display().to_string()),
                    ("RUNE_BRIDGE_WASM_PATH", rune_bridge.display().to_string()),
                ],
                $trycmd_path,
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
    test_should_deploy_brc20_bridge,
    "brc20_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_canister/deploy")
);
test_deploy!(
    test_should_deploy_btc_bridge,
    "btc_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_canister/deploy")
);
test_deploy!(
    test_should_deploy_erc20_bridge,
    "erc20_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_canister/deploy")
);
test_deploy!(
    test_should_deploy_icrc2_bridge,
    "icrc2_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_canister/deploy")
);
test_deploy!(
    test_should_deploy_rune_bridge,
    "rune_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_canister/deploy")
);

// with local node
test_deploy!(
    test_should_deploy_brc20_bridge_evm_rpc,
    "brc20_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_rpc/deploy")
);
test_deploy!(
    test_should_deploy_btc_bridge_evm_rpc,
    "btc_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_rpc/deploy")
);
test_deploy!(
    test_should_deploy_erc20_bridge_evm_rpc,
    "erc20_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_rpc/deploy")
);
test_deploy!(
    test_should_deploy_icrc2_bridge_evm_rpc,
    "icrc2_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_rpc/deploy")
);
test_deploy!(
    test_should_deploy_rune_bridge_evm_rpc,
    "rune_bridge.trycmd",
    Path::new("./tests/bridge_deployer/evm_rpc/deploy")
);

#[tokio::test]
#[serial_test::serial]
#[cfg(feature = "dfx_tests")]
async fn test_should_update_bridge() {
    let ctx = setup(&[
        CanisterType::Evm,
        CanisterType::Signature,
        CanisterType::EvmRpcCanister,
        CanisterType::ExternalEvm,
        CanisterType::Kyt,
        CanisterType::CkBtcLedger,
        CanisterType::CkBtcMinter,
        CanisterType::Icrc2Bridge,
        CanisterType::Erc20Bridge,
        CanisterType::BtcBridge,
        CanisterType::RuneBridge,
        CanisterType::Brc20Bridge,
    ])
    .await;

    let CommonCliArgs {
        evm: evm_principal,
        private_key,
        identity_path,
        evm_node: _evm_node,
        evm_rpc,
    } = CommonCliArgs::new(&ctx).await;

    let DeployCliArgs {
        brc20_bridge,
        btc_bridge,
        erc20_bridge,
        icrc2_bridge,
        rune_bridge,
        ..
    } = DeployCliArgs::new(&ctx).await;

    let admin_principal = ctx.admin().to_text();

    let brc20_bridge_id = ctx.canisters().brc20_bridge().to_text();
    let btc_bridge_id = ctx.canisters().btc_bridge().to_text();
    let erc20_bridge_id = ctx.canisters().erc20_bridge().to_text();
    let icrc2_bridge_id = ctx.canisters().icrc2_bridge().to_text();
    let rune_bridge_id = ctx.canisters().rune_bridge().to_text();

    // get the output dir for evaluated trycmd files
    let trycmd_output_dir = TempDir::new().expect("failed to create temp file");

    // eval the trycmd files
    eval::eval_trycmd(
        [
            ("IDENTITY_PATH", identity_path.display().to_string()),
            ("PRIVATE_KEY", private_key.to_string()),
            ("ADMIN_PRINCIPAL", admin_principal.to_string()),
            ("EVM_PRINCIPAL", evm_principal.to_string()),
            ("EVM_RPC", evm_rpc.to_string()),
            ("BRC20_BRIDGE_WASM_PATH", brc20_bridge.display().to_string()),
            ("BTC_BRIDGE_WASM_PATH", btc_bridge.display().to_string()),
            ("ERC20_BRIDGE_WASM_PATH", erc20_bridge.display().to_string()),
            ("ICRC2_BRIDGE_WASM_PATH", icrc2_bridge.display().to_string()),
            ("RUNE_BRIDGE_WASM_PATH", rune_bridge.display().to_string()),
            ("BRC20_BRIDGE_ID", brc20_bridge_id.to_string()),
            ("BTC_BRIDGE_ID", btc_bridge_id.to_string()),
            ("ERC20_BRIDGE_ID", erc20_bridge_id.to_string()),
            ("ICRC2_BRIDGE_ID", icrc2_bridge_id.to_string()),
            ("RUNE_BRIDGE_ID", rune_bridge_id.to_string()),
        ],
        Path::new("./tests/bridge_deployer/evm_canister/upgrade"),
        trycmd_output_dir.path(),
        "*.trycmd",
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

#[tokio::test]
#[serial_test::serial]
#[cfg(feature = "dfx_tests")]
async fn test_should_reinstall_bridge() {
    let ctx = setup(&[
        CanisterType::Evm,
        CanisterType::Signature,
        CanisterType::EvmRpcCanister,
        CanisterType::ExternalEvm,
        CanisterType::Kyt,
        CanisterType::CkBtcLedger,
        CanisterType::CkBtcMinter,
        CanisterType::Icrc2Bridge,
        CanisterType::Erc20Bridge,
        CanisterType::BtcBridge,
        CanisterType::RuneBridge,
        CanisterType::Brc20Bridge,
    ])
    .await;

    let CommonCliArgs {
        evm: evm_principal,
        evm_node: _evm_node,
        evm_rpc,
        private_key,
        identity_path,
    } = CommonCliArgs::new(&ctx).await;

    let DeployCliArgs {
        brc20_bridge,
        btc_bridge,
        erc20_bridge,
        icrc2_bridge,
        rune_bridge,
        ..
    } = DeployCliArgs::new(&ctx).await;

    let admin_principal = ctx.admin().to_text();

    let brc20_bridge_id = ctx.canisters().brc20_bridge().to_text();
    let btc_bridge_id = ctx.canisters().btc_bridge().to_text();
    let erc20_bridge_id = ctx.canisters().erc20_bridge().to_text();
    let icrc2_bridge_id = ctx.canisters().icrc2_bridge().to_text();
    let rune_bridge_id = ctx.canisters().rune_bridge().to_text();
    let ckbtc_ledger = ctx.canisters.ckbtc_ledger().to_text();
    let ckbtc_minter = ctx.canisters.ckbtc_minter().to_text();

    // deploy the btf bridge
    let btf_bridge = deploy_btf_bridge(&ctx)
        .await
        .expect("failed to deploy btf bridge");

    // get the output dir for evaluated trycmd files
    let trycmd_output_dir = TempDir::new().expect("failed to create temp file");

    // eval the trycmd files
    eval::eval_trycmd(
        [
            ("IDENTITY_PATH", identity_path.display().to_string()),
            ("PRIVATE_KEY", private_key.to_string()),
            ("ADMIN_PRINCIPAL", admin_principal.to_string()),
            ("EVM_PRINCIPAL", evm_principal.to_string()),
            ("EVM_RPC", evm_rpc.to_string()),
            ("BRC20_BRIDGE_WASM_PATH", brc20_bridge.display().to_string()),
            ("BTC_BRIDGE_WASM_PATH", btc_bridge.display().to_string()),
            ("ERC20_BRIDGE_WASM_PATH", erc20_bridge.display().to_string()),
            ("ICRC2_BRIDGE_WASM_PATH", icrc2_bridge.display().to_string()),
            ("RUNE_BRIDGE_WASM_PATH", rune_bridge.display().to_string()),
            ("CKBTC_LEDGER", ckbtc_ledger.to_string()),
            ("CKBTC_MINTER", ckbtc_minter.to_string()),
            ("BRC20_BRIDGE_ID", brc20_bridge_id.to_string()),
            ("BTC_BRIDGE_ID", btc_bridge_id.to_string()),
            ("ERC20_BRIDGE_ID", erc20_bridge_id.to_string()),
            ("ICRC2_BRIDGE_ID", icrc2_bridge_id.to_string()),
            ("RUNE_BRIDGE_ID", rune_bridge_id.to_string()),
            ("BTF_BRIDGE", hex::encode(btf_bridge)),
        ],
        Path::new("./tests/bridge_deployer/evm_canister/reinstall"),
        trycmd_output_dir.path(),
        "*.trycmd",
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

    std::env::set_current_dir(path).expect("failed to change cwd");

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

/// Deploy the BTF bridge
async fn deploy_btf_bridge(ctx: &DfxTestContext) -> anyhow::Result<H160> {
    let private_key_bytes = hex::decode(HARDHAT_ETH_PRIVATE_KEY)?;
    let wallet = Wallet::from_bytes(&private_key_bytes)?;

    let btc_bridge_eth_address = ctx
        .rune_bridge_client(ADMIN)
        .get_bridge_canister_evm_address()
        .await?;

    let wrapped_token_deployer = ctx
        .initialize_wrapped_token_deployer_contract(&wallet)
        .await?;

    let btf_bridge = ctx
        .initialize_btf_bridge_with_minter(
            &wallet,
            btc_bridge_eth_address.unwrap(),
            None,
            wrapped_token_deployer,
            true,
        )
        .await?;

    Ok(btf_bridge.0)
}
