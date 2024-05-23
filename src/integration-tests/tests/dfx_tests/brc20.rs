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

use brc20_bridge::interface::bridge_api::{DepositBrc20Args, DepositError, Erc20MintStatus};
use brc20_bridge::state::{BftBridgeConfig, Brc20BridgeConfig};
use brc20_bridge::{GetAddressError, InscriptionId};
use candid::Principal;
use did::{TransactionReceipt, H160, H256};
use eth_signer::sign_strategy::{SigningKeyId, SigningStrategy};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_log::LogSettings;
use minter_contract_utils::evm_link::EvmLink;
use minter_did::id256::Id256;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::Instant;

use crate::context::{CanisterType, TestContext};
use crate::dfx_tests::{DfxTestContext, ADMIN};
use crate::utils::wasm::get_brc20_bridge_canister_bytecode;

const ORD_DATA_DIR: &str = "target/ord";
const BRC20_DATA: &str = "ord-testnet/brc20_json_inscriptions/brc20_deploy.json";
const BRC20_TICKER: &str = "demo";
const POSTAGE: u128 = 10000;

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

        let inscription_id = Self::get_inscription_details(0).0.get_raw();
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

    // returns the inscription ID and the reveal transaction ID.
    async fn create_brc20_inscription(&self, filename: &str) -> (String, String) {
        let output = self
            .ord_wallet_run(&["inscribe", "--fee-rate", "10", "--file", filename])
            .await;

        eprintln!("{output}");
        let json =
            serde_json::from_str::<Value>(&output).expect("failed to parse ord wallet response");

        self.create_blocks(1).await;

        let iid = json["inscriptions"][0]["id"]
            .as_str()
            .expect("invalid inscription ID");
        let reveal_txid = json["reveal"]
            .as_str()
            .expect("invalid reveal transaction ID");
        (iid.to_owned(), reveal_txid.to_owned())
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

    async fn wrapped_token_balance(&self, wallet: &Wallet<'_, SigningKey>) -> u128 {
        self.inner
            .check_erc20_balance(&self.token_contract, wallet)
            .await
            .expect("Failed to get wrapped token balance")
    }

    async fn ordinal_balance(&self) -> u128 {
        let json = serde_json::from_str::<Value>(&self.ord_wallet_run(&["balance"]).await)
            .expect("failed to parse ordinal balance response");

        (json["ordinal"].as_u64().unwrap_or_else(|| {
            panic!(
                "invalid ordinal balance value: {}. Full json: {json}",
                json["ordinal"]
            )
        })) as u128
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

    // returns the inscription ID and the actual inscription amount (a.k.a, postage).
    fn get_inscription_details(index: usize) -> (InscriptionId, u64) {
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
        let iid =
            InscriptionId::parse_from_str(iid).expect("Failed to parse InscriptionId from string");

        let postage = json[index]["postage"]
            .as_u64()
            .expect("invalid postage amount");

        (iid, postage)
    }

    async fn deposit_brc20(
        &self,
        brc20: DepositBrc20Args,
        eth_address: &H160,
    ) -> Result<Erc20MintStatus, DepositError> {
        const MAX_RETRIES: u32 = 3;
        let mut retry_count = 0;
        while retry_count < MAX_RETRIES {
            match self
                .inner
                .client(self.brc20_bridge(), ADMIN)
                .update::<_, Result<Vec<Erc20MintStatus>, DepositError>>(
                    "brc20_to_erc20",
                    (brc20.clone(), eth_address),
                )
                .await
                .expect("canister call failed")
            {
                Err(DepositError::NothingToDeposit) => retry_count += 1,
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

        Err(DepositError::NothingToDeposit)
    }

    async fn withdraw_brc20(&self, amount: u128) {
        let withdrawal_address = self.get_withdrawal_address().await;
        let client = self.inner.evm_client(ADMIN);
        self.inner
            .burn_erc_20_tokens_raw(
                &client,
                &self.eth_wallet,
                &self.token_contract,
                withdrawal_address.as_bytes().to_vec(),
                &self.bft_bridge_contract,
                amount,
            )
            .await
            .expect("failed to burn wrapped token");

        self.inner.advance_time(Duration::from_secs(15)).await;
        self.create_blocks(1).await;
        self.inner.advance_time(Duration::from_secs(5)).await;

        // Ord indexer doesn't catch the new balance for some reason after the first block, so
        // we mint one more time to make sure indexer is up to date.
        self.create_blocks(1).await;
        self.inner.advance_time(Duration::from_secs(5)).await;
    }

    async fn brc20_to_erc20(&self, iid: &str, tx_id: &str, wallet: &Wallet<'_, SigningKey>) {
        let balance_before = self.wrapped_token_balance(wallet).await;

        let wallet_address = wallet.address();
        let address = self.get_deposit_address(&wallet_address.into()).await;

        let inscription_index = InscriptionId::parse_from_str(iid)
            .ok()
            .expect("invalid inscription ID")
            .index as usize;

        let deposit_args = DepositBrc20Args {
            tx_id: tx_id.to_owned(),
            ticker: BRC20_TICKER.to_string(),
        };

        self.send_inscription(&address, &iid).await;
        self.inner.advance_time(Duration::from_secs(5)).await;
        let postage = Self::get_inscription_details(inscription_index).1 as u128;

        self.send_btc(&address, 490000).await;
        self.inner.advance_time(Duration::from_secs(5)).await;

        self.deposit_brc20(deposit_args, &wallet_address.into())
            .await
            .expect("failed to deposit BRC20 inscription");

        let balance_after = self.wrapped_token_balance(wallet).await;
        assert_eq!(balance_after - balance_before, postage, "Wrapped token balance of the wallet changed by unexpected amount. Balance before: {balance_before}, balance_after: {balance_after}, deposit amount: {postage}");
    }
}

#[tokio::test]
async fn brc20_bridging_flow_for_one_user() {
    let ctx = Brc20TestContext::new().await;
    // Mint one block in case there are some pending transactions
    ctx.create_blocks(1).await;

    let (iid, tx_id) = ctx.create_brc20_inscription(BRC20_DATA).await;
    let ordinal_bal_before_deposit = ctx.ordinal_balance().await;
    // each postage = 10000 sats
    assert_eq!(ordinal_bal_before_deposit, POSTAGE);

    ctx.brc20_to_erc20(&iid, &tx_id, &ctx.eth_wallet).await;
    let updated_wrapped_balance = ctx.wrapped_token_balance(&ctx.eth_wallet).await;
    assert_eq!(updated_wrapped_balance, POSTAGE);

    let ordinal_bal_after_deposit = ctx.ordinal_balance().await;
    assert_eq!(ordinal_bal_after_deposit, 0);

    // each postage = 10000 sats
    ctx.withdraw_brc20(POSTAGE).await;

    let updated_wrapped_balance = ctx.wrapped_token_balance(&ctx.eth_wallet).await;
    assert_eq!(updated_wrapped_balance, 0);

    ctx.stop_canister_instances().await
}

#[tokio::test]
async fn brc20_bridging_flow_for_multiple_users() {
    let ctx = Brc20TestContext::new().await;
    // Create one block in case there are pending transactions
    ctx.create_blocks(1).await;

    let (iid, tx_id) = ctx.create_brc20_inscription(BRC20_DATA).await;
    let ordinal_bal_before_deposit = ctx.ordinal_balance().await;
    // each postage = 10000 sats
    assert_eq!(ordinal_bal_before_deposit, 10000);

    ctx.brc20_to_erc20(&iid, &tx_id, &ctx.eth_wallet).await;

    let ordinal_bal_after_deposit = ctx.ordinal_balance().await;
    assert_eq!(ordinal_bal_after_deposit, 0);

    let (iid, tx_id) = ctx.create_brc20_inscription(BRC20_DATA).await;
    let another_wallet = ctx
        .inner
        .new_wallet(u128::MAX)
        .await
        .expect("failed to create an ETH wallet");
    ctx.brc20_to_erc20(&iid, &tx_id, &another_wallet).await;

    let updated_wrapped_balance = ctx.wrapped_token_balance(&ctx.eth_wallet).await;
    assert_eq!(updated_wrapped_balance, 10000);
    let updated_wrapped_balance = ctx.wrapped_token_balance(&another_wallet).await;
    assert_eq!(updated_wrapped_balance, 10000);

    ctx.withdraw_brc20(10000).await;

    let updated_wrapped_balance = ctx.wrapped_token_balance(&ctx.eth_wallet).await;
    assert_eq!(updated_wrapped_balance, 0);

    assert_eq!(ctx.wrapped_token_balance(&another_wallet).await, 10000);

    ctx.stop_canister_instances().await
}
