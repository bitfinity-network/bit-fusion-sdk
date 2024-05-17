use std::io::ErrorKind;
use std::str::FromStr;
use std::time::Duration;

use btc_bridge::state::BftBridgeConfig;
use candid::Principal;
use did::{TransactionReceipt, H160, H256};
use eth_signer::sign_strategy::{SigningKeyId, SigningStrategy};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_log::LogSettings;
use minter_contract_utils::evm_link::EvmLink;
use rune_bridge::interface::{DepositError, Erc20MintStatus, GetAddressError};
use rune_bridge::rune_info::{RuneInfo, RuneName};
use rune_bridge::state::RuneBridgeConfig;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::Instant;

use crate::context::{CanisterType, TestContext};
use crate::dfx_tests::{DfxTestContext, ADMIN};
use crate::utils::wasm::get_rune_bridge_canister_bytecode;

const RUNE_NAME: &str = "SUPERMAXRUNENAME";
const RUNE_DATA_DIR: &str = "target/ord";
const RUNE_SERVER_URL: &str = "http://localhost:8000";

struct RunesContext {
    inner: DfxTestContext,
    eth_wallet: Wallet<'static, SigningKey>,
    token_contract: H160,
    bft_bridge_contract: H160,
}

fn get_rune_info(name: &str) -> RuneInfo {
    let output = std::process::Command::new("ord")
        .args(["-r", "--data-dir", RUNE_DATA_DIR, "--index-runes", "runes"])
        .output()
        .expect("failed to run 'ord' cli tool");
    if !output.status.success() {
        panic!(
            "'ord' list runes command exited with status {}",
            output.status
        )
    }

    let json =
        serde_json::from_slice::<Value>(&output.stdout).expect("failed to parse ord runes list");
    let id_str = json["runes"][name]["id"].as_str().expect("invalid rune id");
    let id_parts = id_str.split(':').collect::<Vec<_>>();
    RuneInfo {
        name: RuneName::from_str(name).unwrap_or_else(|_| panic!("invalid rune name: {name}")),
        decimals: 8,
        block: u64::from_str(id_parts[0]).unwrap_or_else(|_| panic!("invalid rune id: {id_str}")),
        tx: u32::from_str(id_parts[1]).unwrap_or_else(|_| panic!("invalid rune id: {id_str}")),
    }
}

impl RunesContext {
    async fn new() -> Self {
        let context = DfxTestContext::new(&CanisterType::RUNE_CANISTER_SET).await;

        let bridge = context.canisters().rune_bridge();
        let init_args = RuneBridgeConfig {
            network: BitcoinNetwork::Regtest,
            evm_link: EvmLink::Ic(context.canisters().evm()),
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: SigningKeyId::Dfx,
            },
            admin: context.admin(),
            log_settings: LogSettings {
                enable_console: true,
                in_memory_records: None,
                log_filter: Some("trace".to_string()),
            },
            min_confirmations: 1,
            indexer_url: "https://localhost:8001".to_string(),
            deposit_fee: 500_000,
        };
        context
            .install_canister(
                bridge,
                get_rune_bridge_canister_bytecode().await,
                (init_args,),
            )
            .await
            .unwrap();
        let _: () = context
            .client(bridge, ADMIN)
            .update("admin_configure_ecdsa", ())
            .await
            .unwrap();

        let wallet = context.new_wallet(u128::MAX).await.unwrap();

        let btc_bridge_eth_address: Option<H160> = context
            .client(bridge, ADMIN)
            .update("get_evm_address", ())
            .await
            .unwrap();

        let client = context.evm_client(ADMIN);
        client
            .mint_native_tokens(btc_bridge_eth_address.clone().unwrap(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();

        let bft_bridge = context
            .initialize_bft_bridge_with_minter(&wallet, btc_bridge_eth_address.unwrap())
            .await
            .unwrap();

        let rune_info = get_rune_info(RUNE_NAME);
        let token = context
            .create_wrapped_token(&wallet, &bft_bridge, rune_info.id().into())
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
            .client(bridge, ADMIN)
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

    fn bridge(&self) -> Principal {
        self.inner.canisters().rune_bridge()
    }

    fn eth_wallet_address(&self) -> H160 {
        self.eth_wallet.address().into()
    }

    async fn get_deposit_address(&self, eth_address: &H160) -> String {
        self.inner
            .client(self.bridge(), ADMIN)
            .query::<_, Result<String, GetAddressError>>("get_deposit_address", (eth_address,))
            .await
            .expect("canister call failed")
            .expect("get_deposit_address error")
    }

    async fn send_ord(&self, btc_address: &str, amount: u128) {
        let output = self
            .run_ord(&[
                "send",
                "--fee-rate",
                "10",
                btc_address,
                &format!("{}:{RUNE_NAME}", amount as f64 / 100.0),
            ])
            .await;

        eprintln!("{output}");

        self.mint_blocks(1).await;
    }

    async fn send_btc(&self, btc_address: &str, amount: u64) {
        let output = self
            .run_ord(&[
                "send",
                "--fee-rate",
                "10",
                btc_address,
                &format!("{} btc", amount as f32 / 100_000_000.0),
            ])
            .await;

        eprintln!("{output}");

        self.mint_blocks(1).await;
    }

    async fn run_ord(&self, args: &[&str]) -> String {
        let output = Command::new("ord")
            .envs([
                ("ORD_BITCOIN_RPC_USERNAME", "ic-btc-integration"),
                (
                    "ORD_BITCOIN_RPC_PASSWORD",
                    "QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E=",
                ),
            ])
            .args([
                "-r",
                "--data-dir",
                RUNE_DATA_DIR,
                "--index-runes",
                "wallet",
                "--server-url",
                RUNE_SERVER_URL,
            ])
            .args(args)
            .output()
            .await;

        let result = match output {
            Ok(res) if res.status.success() => res.stdout,
            Err(err) if err.kind() == ErrorKind::NotFound => panic!("`ord` cli tool not found"),
            Err(err) => panic!("'ord' execution failed: {err:?}"),
            Ok(res) => panic!(
                "'ord' exited with status code {}: {} {}",
                res.status,
                String::from_utf8_lossy(&res.stdout),
                String::from_utf8_lossy(&res.stderr),
            ),
        };

        String::from_utf8(result).expect("Ord returned not valid utf8 string")
    }

    async fn mint_blocks(&self, count: u32) {
        // Await all previous operations to synchronize for ord and dfx
        self.inner.advance_time(Duration::from_secs(1)).await;

        let pwd = std::env::var("PWD").expect("PWD is not set");
        let output = Command::new("bitcoin-core.cli")
            .args([
                &format!("-conf={pwd}/btc-deploy/bitcoin.conf"),
                "-rpcwallet=admin",
                "generatetoaddress",
                &count.to_string(),
                "bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts",
            ])
            .output()
            .await;

        let result = match output {
            Ok(out) if out.status.success() => {
                String::from_utf8(out.stdout).expect("invalid bitcoin-cli output")
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                panic!("`bitcoin-core.cli` cli tool not found")
            }
            Err(err) => panic!("'ord' execution failed: {err:?}"),
            Ok(out) => panic!("'ord' exited with status code {}", out.status),
        };

        eprintln!("{}", result);

        // Allow dfx and ord catch up with the new block
        self.inner.advance_time(Duration::from_secs(5)).await;
    }

    async fn deposit(&self, eth_address: &H160) -> Result<Erc20MintStatus, DepositError> {
        const MAX_RETRIES: u32 = 3;
        let mut retry_count = 0;
        while retry_count < MAX_RETRIES {
            match self
                .inner
                .client(self.bridge(), ADMIN)
                .update::<_, Result<Vec<Erc20MintStatus>, DepositError>>("deposit", (eth_address,))
                .await
                .expect("canister call failed")
            {
                Err(DepositError::NotingToDeposit) | Err(DepositError::NotEnoughBtc { .. }) => {
                    retry_count += 1
                }
                Ok(statuses) => match &statuses[0] {
                    res @ Erc20MintStatus::Minted { ref tx_id, .. } => {
                        self.inner.advance_time(Duration::from_secs(2)).await;
                        self.wait_for_tx_success(tx_id).await;

                        return Ok(res.clone());
                    }
                    status => return Ok(status.clone()),
                },
                result => {
                    return Err(DepositError::Unavailable(format!(
                        "Unexpected deposit result: {result:?}"
                    )))
                }
            }
        }

        Err(DepositError::NotingToDeposit)
    }

    async fn wait_for_tx_success(&self, tx_hash: &H256) -> TransactionReceipt {
        const MAX_TX_TIMEOUT_SEC: u64 = 6;

        let start = Instant::now();
        let timeout = Duration::from_secs(MAX_TX_TIMEOUT_SEC);
        let client = self.inner.evm_client(ADMIN);
        while start.elapsed() < timeout {
            let receipt = client
                .eth_get_transaction_receipt(tx_hash.clone())
                .await
                .expect("Failed to request transaction receipt")
                .expect("Request for receipt failed");

            if let Some(receipt) = receipt {
                if receipt.status != Some(1u64.into()) {
                    eprintln!("Transaction: {tx_hash}");
                    eprintln!("Receipt: {receipt:?}");
                    if let Some(output) = receipt.output {
                        let output = String::from_utf8_lossy(&output);
                        eprintln!("Output: {output}");
                    }

                    panic!("Transaction failed");
                } else {
                    return receipt;
                }
            } else {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        panic!("Transaction {tx_hash} timed out");
    }

    async fn stop(&self) {
        self.inner
            .stop_canister(self.inner.canisters().evm())
            .await
            .expect("Failed to stop evm canister");
        self.inner
            .stop_canister(self.inner.canisters().rune_bridge())
            .await
            .expect("Failed to stop rune bridge canister");
    }

    async fn withdraw(&self, amount: u128) {
        let withdrawal_address = self.get_withdrawal_address().await;
        self.inner
            .burn_erc_20_tokens_raw(
                &self.inner.evm_client(ADMIN),
                &self.eth_wallet,
                &self.token_contract,
                withdrawal_address.as_bytes().to_vec(),
                &self.bft_bridge_contract,
                amount,
            )
            .await
            .expect("failed to burn wrapped token");

        self.inner.advance_time(Duration::from_secs(15)).await;
        self.mint_blocks(1).await;
        self.inner.advance_time(Duration::from_secs(5)).await;
    }

    async fn get_withdrawal_address(&self) -> String {
        let json = serde_json::from_str::<Value>(&self.run_ord(&["receive"]).await)
            .expect("failed to parse ord balance response");

        json["addresses"][0]
            .as_str()
            .expect("invalid address value")
            .to_string()
    }

    async fn wrapped_balance(&self, wallet: &Wallet<'_, SigningKey>) -> u128 {
        self.inner
            .check_erc20_balance(&self.token_contract, wallet)
            .await
            .expect("Failed to get wrapped token balance")
    }

    async fn ord_rune_balance(&self) -> u128 {
        let json = serde_json::from_str::<Value>(&self.run_ord(&["balance"]).await)
            .expect("failed to parse ord balance response");

        (json["runes"][RUNE_NAME]
            .as_str()
            .unwrap_or_else(|| {
                panic!(
                    "invalid balance value: {}. Full json: {json}",
                    json["runes"][RUNE_NAME]
                )
            })
            .parse::<f64>()
            .unwrap_or_else(|_| {
                panic!(
                    "invalid balance value: {}. Full json: {json}",
                    json["runes"][RUNE_NAME]
                )
            })
            * 100.0) as u128
    }
}

#[tokio::test]
async fn runes_bridging_flow() {
    let ctx = RunesContext::new().await;

    // Mint one block in case there are some pending transactions
    ctx.mint_blocks(1).await;

    let ord_balance = ctx.ord_rune_balance().await;

    let wallet_address = ctx.eth_wallet_address();
    let address = ctx.get_deposit_address(&wallet_address).await;

    ctx.send_ord(&address, 100).await;
    ctx.send_btc(&address, 490000).await;

    ctx.inner.advance_time(Duration::from_secs(5)).await;

    ctx.deposit(&wallet_address)
        .await
        .expect("failed to deposit runes");

    let balance = ctx.wrapped_balance(&ctx.eth_wallet).await;
    assert_eq!(balance, 100);

    ctx.withdraw(30).await;

    let updated_balance = ctx.wrapped_balance(&ctx.eth_wallet).await;
    assert_eq!(updated_balance, 70);

    let updated_ord_balance = ctx.ord_rune_balance().await;

    assert_eq!(updated_ord_balance, ord_balance - 70);

    ctx.stop().await
}
