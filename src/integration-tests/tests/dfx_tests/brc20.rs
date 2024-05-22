#![allow(dead_code)]
// NOTE:
//
// The command `ord env <DATA_DIRECTORY>`
// starts a regtest `ord` and `bitcoind` instance, waiting for further commands.
//
// Therefore, before executing these tests, kindly fulfill the following:
//      1. ensure that you have `ord` installed
//      2. ensure that you're at the root of this crate
//      3. run `ord env target/ord` in a separate terminal instance

use std::io::ErrorKind;
use std::time::Duration;

use brc20_bridge::state::{BftBridgeConfig, Brc20BridgeConfig};
use brc20_bridge::{GetAddressError, InscriptionId};
use candid::Principal;
use did::H160;
use eth_signer::sign_strategy::{SigningKeyId, SigningStrategy};
use eth_signer::Wallet;
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_log::LogSettings;
use minter_contract_utils::evm_link::EvmLink;
use minter_did::id256::Id256;
use serde_json::Value;
use tokio::process::Command;

use crate::context::{CanisterType, TestContext};
use crate::dfx_tests::{DfxTestContext, ADMIN};
use crate::utils::wasm::get_brc20_bridge_canister_bytecode;

const ORD_DATA_DIR: &str = "target/ord";

struct Brc20TestContext {
    inner: DfxTestContext,
    eth_wallet: Wallet<'static, SigningKey>,
    token_contract: H160,
    bft_bridge_contract: H160,
}

impl Brc20TestContext {
    async fn new() -> Self {
        let context = DfxTestContext::new(&CanisterType::BRC20_CANISTER_SET).await;

        let brc20_bridge = context.canisters().brc20_bridge();
        let brc20_bridge_config = Brc20BridgeConfig {
            network: BitcoinNetwork::Regtest,
            evm_link: EvmLink::Ic(context.canisters().evm()),
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: SigningKeyId::Dfx,
            },
            admin: context.admin(),
            deposit_fee: 500_000,
            general_indexer: "https://localhost:9001".to_string(),
            brc20_indexer: "https://localhost:8001".to_string(),
            logger: LogSettings {
                enable_console: true,
                in_memory_records: None,
                log_filter: Some("trace".to_string()),
            },
        };
        context
            .install_canister(
                brc20_bridge,
                get_brc20_bridge_canister_bytecode().await,
                (brc20_bridge_config,),
            )
            .await
            .unwrap();
        let _: () = context
            .client(brc20_bridge, ADMIN)
            .update("admin_configure_ecdsa", ())
            .await
            .unwrap();

        let wallet = context.new_wallet(u128::MAX).await.unwrap();

        let brc20_bridge_eth_address: Option<H160> = context
            .client(brc20_bridge, ADMIN)
            .update("get_evm_address", ())
            .await
            .unwrap();

        let client = context.evm_client(ADMIN);
        client
            .mint_native_tokens(brc20_bridge_eth_address.clone().unwrap(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();

        let bft_bridge = context
            .initialize_bft_bridge_with_minter(&wallet, brc20_bridge_eth_address.unwrap())
            .await
            .unwrap();

        let inscription_id = Self::get_inscription_id(0).get_raw();
        let iid_256 =
            Id256::from_slice(&inscription_id).expect("Failed to convert inscription_id to Id256");
        let token = context
            .create_wrapped_token(&wallet, &bft_bridge, iid_256)
            .await
            .unwrap();

        let chain_id = context.evm_client(ADMIN).eth_chain_id().await.unwrap();

        let mut token_name = [0; 32];
        token_name[0..7].copy_from_slice(b"wrapper");
        let mut token_symbol = [0; 16];
        token_symbol[0..3].copy_from_slice(b"WPT");

        let bft_config = BftBridgeConfig {
            erc20_chain_id: chain_id as u32,
            bridge_address: bft_bridge.clone(),
            token_address: token.clone(),
            token_name,
            token_symbol,
            decimals: 0,
        };

        let _: () = context
            .client(brc20_bridge, ADMIN)
            .update("admin_configure_bft_bridge", (bft_config,))
            .await
            .unwrap();

        context.advance_time(Duration::from_secs(2)).await;

        Self {
            inner: context,
            eth_wallet: wallet,
            token_contract: token,
            bft_bridge_contract: bft_bridge,
        }
    }

    fn brc20_bridge(&self) -> Principal {
        self.inner.canisters().brc20_bridge()
    }

    async fn stop_canister_instances(&self) {
        self.inner
            .stop_canister(self.inner.canisters().evm())
            .await
            .expect("Failed to stop evm canister");
        self.inner
            .stop_canister(self.inner.canisters().brc20_bridge())
            .await
            .expect("Failed to stop brc20 bridge canister");
    }

    async fn ord_wallet_run(&self, args: &[&str]) -> String {
        let output = Command::new("ord")
            .args(["--datadir", ORD_DATA_DIR, "wallet"])
            .args(args)
            .output()
            .await;

        let result = match output {
            Ok(res) if res.status.success() => res.stdout,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                panic!("`ord` cli tool not found")
            }
            Err(err) => panic!("`ord` wallet command failed: {err:?}"),
            Ok(res) => panic!(
                "`ord` exited with status code {}: {} {}",
                res.status,
                String::from_utf8_lossy(&res.stdout),
                String::from_utf8_lossy(&res.stderr),
            ),
        };

        String::from_utf8(result).expect("Invalid UTF-8")
    }

    async fn bitcoin_cli_run(&self, args: &[&str]) -> Result<std::process::Output, std::io::Error> {
        let datadir = format!("-datadir='{}'", ORD_DATA_DIR);
        Command::new("bitcoin-cli")
            .arg(&datadir)
            .args(args)
            .output()
            .await
    }

    async fn get_deposit_address(&self, eth_address: &H160) -> String {
        self.inner
            .client(self.brc20_bridge(), ADMIN)
            .query::<_, Result<String, GetAddressError>>("get_deposit_address", (eth_address,))
            .await
            .expect("canister call failed")
            .expect("get_deposit_address error")
    }

    async fn get_withdrawal_address(&self) -> String {
        let json = serde_json::from_str::<Value>(&self.ord_wallet_run(&["receive"]).await)
            .expect("failed to parse ord wallet response");

        json["addresses"][0]
            .as_str()
            .expect("invalid bitcoin address")
            .to_string()
    }

    async fn send_inscription(&self, dst_addr: &str, inscription_id: &str) {
        let output = self
            .ord_wallet_run(&["send", "--fee-rate", "10", dst_addr, inscription_id])
            .await;

        eprintln!("{output}");

        self.create_blocks(1).await;
    }

    async fn send_btc(&self, dst_addr: &str, amount: u64) {
        let output = self
            .ord_wallet_run(&[
                "send",
                "--fee-rate",
                "10",
                dst_addr,
                &format!("{} btc", amount as f32 / 100_000_000.0),
            ])
            .await;

        eprintln!("{output}");

        self.create_blocks(1).await;
    }

    async fn create_blocks(&self, count: u32) {
        // Await all previous operations to synchronize for ord and dfx
        self.inner.advance_time(Duration::from_secs(1)).await;

        let output = self
            .bitcoin_cli_run(&[
                "generatetoaddress",
                &count.to_string(),
                "bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts",
            ])
            .await;

        let result = match output {
            Ok(out) if out.status.success() => {
                String::from_utf8(out.stdout).expect("invalid bitcoin-cli output")
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                panic!("`bitcoin-cli` not found. Ensure `ord` is running a regtest `bitcoind` instance.")
            }
            Err(err) => panic!("failed to create blocks: {err:?}"),
            Ok(out) => panic!("`bitcoin-cli` exited with status code {}", out.status),
        };

        eprintln!("{}", result);

        // Allow `dfx` and `ord` to catch up with the new block
        self.inner.advance_time(Duration::from_secs(5)).await;
    }

    fn get_inscription_id(index: usize) -> InscriptionId {
        let output = std::process::Command::new("ord")
            .args(["-r", "--data-dir", ORD_DATA_DIR, "wallet", "inscriptions"])
            .output()
            .expect("failed to run 'ord' cli tool");
        if !output.status.success() {
            panic!(
                "'ord' list inscriptions command exited with status {}",
                output.status
            )
        }

        let json = serde_json::from_slice::<Value>(&output.stdout)
            .expect("failed to parse list of ord inscriptions");
        let iid = json[index]["inscription"]
            .as_str()
            .expect("invalid inscription id");

        InscriptionId::parse_from_str(iid).expect("Failed to parse InscriptionId from string")
    }
}
