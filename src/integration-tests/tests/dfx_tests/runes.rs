use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use alloy_sol_types::SolCall;
use bitcoin::key::Secp256k1;
use bitcoin::{Address, Amount, PrivateKey, Txid};
use bridge_client::BridgeCanisterClient;
use bridge_did::event_data::MinterNotificationType;
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{RuneBridgeDepositOp, RuneBridgeOp};
use bridge_did::runes::RuneName;
use bridge_utils::BTFBridge;
use candid::{Encode, Principal};
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::{BlockNumber, TransactionReceipt, H160, H256};
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ord_rs::Utxo;
use ordinals::{Etching, Rune, RuneId, Terms};
use rune_bridge::interface::{DepositError, GetAddressError};
use rune_bridge::ops::RuneDepositRequestData;
use tokio::time::Instant;

use crate::context::{CanisterType, TestContext};
use crate::dfx_tests::{block_until_succeeds, DfxTestContext, ADMIN};
use crate::utils::btc_rpc_client::BitcoinRpcClient;
use crate::utils::miner::{Exit, Miner};
use crate::utils::ord_client::OrdClient;
use crate::utils::rune_helper::RuneHelper;

const REQUIRED_CONFIRMATIONS: u64 = 6;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum RuneDepositStrategy {
    AllInOne,
    OnePerTx,
}

struct RuneWalletInfo {
    id256: Id256,
    name: String,
}

struct RuneWallet {
    admin_address: Address,
    admin_btc_rpc_client: Arc<BitcoinRpcClient>,
    ord_wallet: BtcWallet,
    runes: HashMap<RuneId, RuneWalletInfo>,
}

struct RunesContext {
    inner: DfxTestContext,
    eth_wallet: Wallet<'static, SigningKey>,
    btf_bridge_contract: H160,
    exit: Exit,
    miner: Option<JoinHandle<()>>,
    runes: RuneWallet,
    tokens: HashMap<RuneId, H160>,
}

fn generate_rune_name() -> String {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let mut name = String::new();
    for _ in 0..16 {
        name.push(rng.gen_range(b'A'..=b'Z') as char);
    }
    name
}

/// Setup a new rune for DFX tests
async fn dfx_rune_setup(runes_to_etch: &[String]) -> anyhow::Result<RuneWallet> {
    let rune_name = generate_rune_name();
    let admin_btc_rpc_client = BitcoinRpcClient::dfx_test_client(&rune_name);
    let admin_address = admin_btc_rpc_client.get_new_address()?;

    // create ord wallet
    let ord_wallet = generate_btc_wallet();

    let mut runes = HashMap::new();

    for rune_name in runes_to_etch {
        // 0.1 BTC => 10_000_000 sat
        let commit_fund_tx;
        loop {
            match admin_btc_rpc_client
                .send_to_address(&ord_wallet.address, Amount::from_sat(10_000_000))
            {
                Ok(tx) => {
                    commit_fund_tx = tx;
                    break;
                }
                Err(err) => {
                    println!("Failed to send btc: {err}");
                    admin_btc_rpc_client.generate_to_address(&admin_address, 10)?;
                }
            }
        }
        admin_btc_rpc_client.generate_to_address(&admin_address, 1)?;

        let commit_utxo =
            admin_btc_rpc_client.get_utxo_by_address(&commit_fund_tx, &ord_wallet.address)?;

        // etch
        let etcher = RuneHelper::new(
            &admin_btc_rpc_client,
            &ord_wallet.private_key,
            &ord_wallet.address,
        );
        let etching = Etching {
            rune: Some(Rune::from_str(rune_name).unwrap()),
            divisibility: Some(2),
            premine: Some(1_000_000),
            spacers: None,
            symbol: Some('$'),
            terms: Some(Terms {
                amount: Some(200_000),
                cap: Some(500),
                height: (None, None),
                offset: (None, None),
            }),
            turbo: true,
        };
        let rune_id = etcher.etch(commit_utxo, etching).await?;
        println!("Etched rune id: {rune_id}",);

        let rune_info = RuneWalletInfo {
            id256: rune_id.into(),
            name: rune_name.clone(),
        };

        runes.insert(rune_id, rune_info);
    }

    Ok(RuneWallet {
        admin_btc_rpc_client: Arc::new(admin_btc_rpc_client),
        admin_address,
        ord_wallet,
        runes,
    })
}

impl RunesContext {
    async fn new(runes: &[String]) -> Self {
        let rune_wallet = dfx_rune_setup(runes).await.expect("failed to setup runes");

        let context = DfxTestContext::new(&CanisterType::RUNE_CANISTER_SET).await;
        context
            .evm_client(ADMIN)
            .set_logger_filter("info")
            .await
            .expect("failed to set logger filter")
            .unwrap();

        let bridge = context.canisters().rune_bridge();

        let _: () = context
            .client(bridge, ADMIN)
            .update("admin_configure_ecdsa", ())
            .await
            .unwrap();

        let wallet = context.new_wallet(u128::MAX).await.unwrap();

        let btc_bridge_eth_address = context
            .rune_bridge_client(ADMIN)
            .get_bridge_canister_evm_address()
            .await
            .unwrap();

        let client = context.evm_client(ADMIN);
        client
            .admin_mint_native_tokens(btc_bridge_eth_address.clone().unwrap(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();

        let wrapped_token_deployer = context
            .initialize_wrapped_token_deployer_contract(&wallet)
            .await
            .unwrap();
        let btf_bridge = context
            .initialize_btf_bridge_with_minter(
                &wallet,
                btc_bridge_eth_address.unwrap(),
                None,
                wrapped_token_deployer,
                true,
            )
            .await
            .unwrap();

        let mut tokens = HashMap::new();

        for rune_id in rune_wallet.runes.keys() {
            let token = context
                .create_wrapped_token(&wallet, &btf_bridge, (*rune_id).into())
                .await
                .unwrap();

            tokens.insert(*rune_id, token);
        }

        let _: () = context
            .rune_bridge_client(ADMIN)
            .set_btf_bridge_contract(&btf_bridge)
            .await
            .unwrap();

        context.advance_time(Duration::from_secs(2)).await;

        let exit = Arc::new(AtomicBool::new(false));
        let miner = Miner::run(
            rune_wallet.admin_address.clone(),
            &rune_wallet.admin_btc_rpc_client,
            &exit,
        );

        Self {
            btf_bridge_contract: btf_bridge,
            eth_wallet: wallet,
            exit,
            miner: Some(miner),
            inner: context,
            runes: rune_wallet,
            tokens,
        }
    }

    fn bridge(&self) -> Principal {
        self.inner.canisters().rune_bridge()
    }

    async fn get_deposit_address(&self, eth_address: &H160) -> String {
        self.inner
            .client(self.bridge(), ADMIN)
            .query::<_, Result<String, GetAddressError>>("get_deposit_address", (eth_address,))
            .await
            .expect("canister call failed")
            .expect("get_deposit_address error")
    }

    async fn send_runes(&self, btc_address: &Address, runes: &[(&RuneId, u128)]) {
        let etcher = RuneHelper::new(
            &self.runes.admin_btc_rpc_client,
            &self.runes.ord_wallet.private_key,
            &self.runes.ord_wallet.address,
        );

        // load utxos
        let mut utxos = Vec::with_capacity(runes.len());
        for (rune_id, _) in runes {
            let rune_info = self.runes.runes.get(rune_id).expect("rune not found");

            // find the utxo
            let balance = OrdClient::dfx_test_client()
                .get_balances(&rune_info.name)
                .await
                .expect("failed to get rune balances");

            for outpoint in balance.keys() {
                let outpoint_info = OrdClient::dfx_test_client()
                    .get_outpoint(outpoint)
                    .await
                    .expect("failed to get outpoint owner");

                let tokens = outpoint.split(':').collect::<Vec<_>>();
                let txid = Txid::from_str(tokens[0]).expect("failed to parse txid");
                let index = tokens[1].parse::<u32>().expect("failed to parse index");

                if outpoint_info.address == self.runes.ord_wallet.address {
                    utxos.push(Utxo {
                        index,
                        id: txid,
                        amount: outpoint_info.value,
                    });
                }
            }
        }

        if utxos.len() < runes.len() {
            panic!("Runes not found; got {utxos:?}; required {runes:?}");
        }

        // get funding utxo
        let edict_fund_tx = self
            .send_btc(&self.runes.ord_wallet.address, Amount::from_sat(10_000_000))
            .await;

        let edict_funds_utxo = self
            .runes
            .admin_btc_rpc_client
            .get_utxo_by_address(&edict_fund_tx, &self.runes.ord_wallet.address)
            .expect("failed to get utxo");

        let mut inputs = utxos;
        inputs.push(edict_funds_utxo);

        let amounts = runes;
        let runes = runes
            .iter()
            .map(|(rune_id, amount)| (**rune_id, *amount))
            .collect::<Vec<_>>();

        let tx_id = etcher
            .edict_runes(inputs, runes, btc_address.clone())
            .await
            .expect("failed to send runes");

        self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;
        println!(
            "{runes_count} Runes sent. txid: {tx_id}; sent to {btc_address}; amounts: {amounts:?}",
            runes_count = amounts.len(),
        );
    }

    async fn send_btc(&self, btc_address: &Address, amount: Amount) -> Txid {
        loop {
            match self
                .runes
                .admin_btc_rpc_client
                .send_to_address(btc_address, amount)
            {
                Err(err) => {
                    println!("Failed to send btc: {err}");
                    self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;
                }
                Ok(tx) => {
                    self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;
                    return tx;
                }
            }
        }
    }

    /// Wait for the specified number of blocks to be mined
    async fn wait_for_blocks(&self, count: u64) {
        let block_height = self
            .runes
            .admin_btc_rpc_client
            .get_block_height()
            .expect("failed to get block count");
        let target = block_height + count;
        while self
            .runes
            .admin_btc_rpc_client
            .get_block_height()
            .expect("failed to get block count")
            < target
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn deposit(&self, runes: &[RuneId], eth_address: &H160) -> Result<(), DepositError> {
        self.send_deposit_notification(runes, eth_address, None)
            .await;

        self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;
        const MAX_WAIT: Duration = Duration::from_secs(60);
        const OP_INTERVAL: Duration = Duration::from_secs(5);
        let start = Instant::now();

        let mut successful_orders = HashSet::new();

        while start.elapsed() < MAX_WAIT {
            println!(
                "Checking deposit status. Elapsed {}s...",
                start.elapsed().as_secs()
            );

            let response: Vec<(OperationId, RuneBridgeOp)> = self
                .inner
                .rune_bridge_client(ADMIN)
                .get_operations_list(eth_address, None, None)
                .await
                .expect("canister call failed");

            for (op_id, op) in &response {
                if matches!(
                    op,
                    RuneBridgeOp::Deposit(RuneBridgeDepositOp::MintOrderConfirmed { .. })
                ) {
                    successful_orders.insert(*op_id);
                    println!(
                        "Deposit confirmed: {op_id}; successful_orders: {}/{}",
                        successful_orders.len(),
                        runes.len()
                    );
                }
            }

            // since we use batched, one is enough
            if !successful_orders.is_empty() {
                return Ok(());
            }
            println!("Deposit response: {response:?}");
            self.inner.advance_time(OP_INTERVAL).await;
        }

        println!("Successful {}/{}", successful_orders.len(), runes.len());

        Err(DepositError::NothingToDeposit)
    }

    async fn send_deposit_notification(
        &self,
        runes: &[RuneId],
        wallet_address: &H160,
        amounts: Option<HashMap<RuneName, u128>>,
    ) {
        let mut dst_tokens = HashMap::new();
        for rune_id in runes {
            let erc20_address = self.tokens.get(rune_id).expect("token not found");
            let rune_info = self.runes.runes.get(rune_id).expect("rune not found");

            dst_tokens.insert(
                RuneName::from_str(&rune_info.name).unwrap(),
                erc20_address.clone(),
            );
        }

        let client = self.inner.evm_client(ADMIN);
        let chain_id = client.eth_chain_id().await.expect("failed to get chain id");
        let nonce = client
            .eth_get_transaction_count(self.eth_wallet.address().into(), BlockNumber::Latest)
            .await
            .unwrap()
            .unwrap();

        let data = RuneDepositRequestData {
            dst_address: wallet_address.clone(),
            dst_tokens,
            amounts,
        };

        let input = BTFBridge::notifyMinterCall {
            notificationType: MinterNotificationType::DepositRequest as u32,
            userData: Encode!(&data).unwrap().into(),
            memo: alloy_sol_types::private::FixedBytes::ZERO,
        }
        .abi_encode();

        let transaction = TransactionBuilder {
            from: &self.eth_wallet.address().into(),
            to: Some(self.btf_bridge_contract.clone()),
            nonce,
            value: Default::default(),
            gas: 5_000_000u64.into(),
            gas_price: Some((EIP1559_INITIAL_BASE_FEE * 2).into()),
            input,
            signature: SigningMethod::SigningKey(self.eth_wallet.signer()),
            chain_id,
        }
        .calculate_hash_and_build()
        .expect("failed to sign the transaction");

        let tx_id = client
            .send_raw_transaction(transaction)
            .await
            .unwrap()
            .unwrap();
        self.wait_for_tx_success(&tx_id).await;
        eprintln!(
            "Deposit notification sent by tx: 0x{}",
            hex::encode(tx_id.0)
        );
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

    async fn withdraw(&self, rune_id: &RuneId, amount: u128) -> anyhow::Result<()> {
        let token_address = self.tokens.get(rune_id).expect("token not found");
        let rune_info = self.runes.runes.get(rune_id).expect("rune not found");

        let withdrawal_address = self.runes.ord_wallet.address.to_string();
        let client = self.inner.evm_client(ADMIN);
        self.inner
            .burn_erc_20_tokens_raw(
                &client,
                &self.eth_wallet,
                token_address,
                rune_info.id256.0.as_slice(),
                withdrawal_address.as_bytes().to_vec(),
                &self.btf_bridge_contract,
                amount,
                true,
                None,
            )
            .await?;

        self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;

        Ok(())
    }

    async fn wrapped_balance(&self, rune_id: &RuneId, wallet: &Wallet<'_, SigningKey>) -> u128 {
        let token_contract = self.tokens.get(rune_id).expect("token not found");

        self.inner
            .check_erc20_balance(token_contract, wallet, None)
            .await
            .expect("Failed to get wrapped token balance")
    }

    async fn wrapped_balances(
        &self,
        runes: &[RuneId],
        wallet: &Wallet<'_, SigningKey>,
    ) -> HashMap<RuneId, u128> {
        let mut balances = HashMap::new();
        for rune_id in runes {
            let balance = self.wrapped_balance(rune_id, wallet).await;
            balances.insert(*rune_id, balance);
        }

        balances
    }

    async fn ord_rune_balance(&self, rune_id: &RuneId) -> u128 {
        let rune_info = self.runes.runes.get(rune_id).expect("rune not found");

        let balance = OrdClient::dfx_test_client()
            .get_balances(&rune_info.name)
            .await
            .expect("failed to get rune balances");
        let mut amount = 0;
        for (outpoint, balance) in balance {
            let owner = OrdClient::dfx_test_client()
                .get_outpoint(&outpoint)
                .await
                .expect("failed to get outpoint owner")
                .address;
            println!("found outpoint {outpoint} with balance {balance} owned by {owner}");
            if owner == self.runes.ord_wallet.address {
                amount += balance as u128;
            }
        }

        amount
    }

    /*
    pub fn mint_blocks(&self, count: u64) {
        self.runes
            .admin_btc_rpc_client
            .generate_to_address(&self.runes.admin_address, count)
            .expect("failed to generate blocks");
    }
     */

    async fn deposit_runes_to(
        &self,
        runes: &[(&RuneId, u128)],
        wallet: &Wallet<'_, SigningKey>,
        deposit_strategy: RuneDepositStrategy,
    ) {
        let rune_ids = runes
            .iter()
            .map(|(rune_id, _)| **rune_id)
            .collect::<Vec<_>>();
        let balance_before = self.wrapped_balances(&rune_ids, wallet).await;

        let wallet_address = wallet.address();
        let address = self.get_deposit_address(&wallet_address.into()).await;
        println!("Wallet address: {wallet_address}; deposit_address {address}");

        let btc_address = Address::from_str(&address)
            .expect("failed to parse btc address")
            .assume_checked();

        match deposit_strategy {
            RuneDepositStrategy::OnePerTx => {
                for rune in runes {
                    self.send_runes(&btc_address, &[*rune]).await;
                    self.send_btc(&btc_address, Amount::from_int_btc(1)).await;
                }
            }
            RuneDepositStrategy::AllInOne => {
                self.send_runes(&btc_address, runes).await;
                self.send_btc(&btc_address, Amount::from_int_btc(1)).await;
            }
        }

        self.deposit(&rune_ids, &wallet_address.into())
            .await
            .expect("failed to deposit runes");

        let balance_after = self.wrapped_balances(&rune_ids, wallet).await;

        for (rune_id, rune_amount) in runes {
            let balance_after = balance_after.get(rune_id).copied().unwrap();
            let balance_before = balance_before.get(rune_id).copied().unwrap();
            assert_eq!(balance_after - balance_before, *rune_amount, "Wrapped token balance of the wallet changed by unexpected amount. Balance before: {balance_before}, balance_after: {balance_after}, deposit amount: {rune_amount}");
        }

        self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;
    }
}

impl Drop for RunesContext {
    fn drop(&mut self) {
        self.exit.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(join) = self.miner.take() {
            join.join().expect("failed to join miner thread");
        }
    }
}

struct BtcWallet {
    private_key: PrivateKey,
    address: Address,
}

fn generate_btc_wallet() -> BtcWallet {
    use rand::Rng as _;
    let entropy = rand::thread_rng().gen::<[u8; 16]>();
    let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap();

    let seed = mnemonic.to_seed("");

    let private_key =
        bitcoin::PrivateKey::from_slice(&seed[..32], bitcoin::Network::Regtest).unwrap();
    let public_key = private_key.public_key(&Secp256k1::new());

    let address = Address::p2wpkh(&public_key, bitcoin::Network::Regtest).unwrap();

    BtcWallet {
        private_key,
        address,
    }
}

#[tokio::test]
async fn runes_bridging_flow() {
    let ctx = RunesContext::new(&[generate_rune_name()]).await;

    let rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let ord_balance = ctx.ord_rune_balance(&rune_id).await;
    ctx.deposit_runes_to(
        &[(&rune_id, 100)],
        &ctx.eth_wallet,
        RuneDepositStrategy::AllInOne,
    )
    .await;

    // withdraw back 30 of rune
    let ctx = Arc::new(ctx);
    let ctx_t = ctx.clone();
    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            Box::pin(async move { ctx_t.withdraw(&rune_id, 30).await })
        },
        Duration::from_secs(60),
    )
    .await;

    ctx.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;

    let updated_balance = ctx.wrapped_balance(&rune_id, &ctx.eth_wallet).await;
    assert_eq!(updated_balance, 70);

    let expected_balance = ord_balance - 100 + 30;

    let ctx_t = ctx.clone();
    // advance
    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            Box::pin(async move {
                let updated_ord_balance = ctx_t.ord_rune_balance(&rune_id).await;
                if updated_ord_balance == expected_balance {
                    return Ok(());
                }

                Err(anyhow::anyhow!(
                    "Expected balance: {expected_balance}; got {updated_ord_balance}"
                ))
            })
        },
        Duration::from_secs(180),
    )
    .await;

    let updated_ord_balance = ctx.ord_rune_balance(&rune_id).await;

    assert_eq!(updated_ord_balance, expected_balance);

    ctx.stop().await
}

#[tokio::test]
async fn inputs_from_different_users() {
    let ctx = RunesContext::new(&[generate_rune_name()]).await;

    let rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let rune_balance = ctx.ord_rune_balance(&rune_id).await;
    ctx.deposit_runes_to(
        &[(&rune_id, 100)],
        &ctx.eth_wallet,
        RuneDepositStrategy::AllInOne,
    )
    .await;

    let another_wallet = ctx
        .inner
        .new_wallet(u128::MAX)
        .await
        .expect("failed to create an ETH wallet");
    ctx.deposit_runes_to(
        &[(&rune_id, 77)],
        &another_wallet,
        RuneDepositStrategy::AllInOne,
    )
    .await;

    let ctx = Arc::new(ctx);
    ctx.withdraw(&rune_id, 50)
        .await
        .expect("failed to withdraw");

    let ctx_t = ctx.clone();

    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            Box::pin(async move {
                let updated_balance = ctx_t.wrapped_balance(&rune_id, &ctx_t.eth_wallet).await;
                if updated_balance == 50 {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Expected balance: 50; got {updated_balance}"
                    ))
                }
            })
        },
        Duration::from_secs(120),
    )
    .await;

    let expected_balance = rune_balance - 50 - 77;

    let ctx_t = ctx.clone();
    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            Box::pin(async move {
                let updated_rune_balance = ctx_t.ord_rune_balance(&rune_id).await;
                if updated_rune_balance == expected_balance {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Expected balance: {expected_balance}; got {updated_rune_balance}"
                    ))
                }
            })
        },
        Duration::from_secs(120),
    )
    .await;

    assert_eq!(ctx.wrapped_balance(&rune_id, &another_wallet).await, 77);
    assert_eq!(ctx.wrapped_balance(&rune_id, &ctx.eth_wallet).await, 50);

    ctx.stop().await
}

#[tokio::test]
async fn test_should_deposit_two_runes_in_a_single_tx() {
    let ctx = RunesContext::new(&[generate_rune_name(), generate_rune_name()]).await;
    let foo_rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    let bar_rune_id = ctx.runes.runes.keys().nth(1).copied().unwrap();

    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let before_balances = ctx
        .wrapped_balances(&[foo_rune_id, bar_rune_id], &ctx.eth_wallet)
        .await;
    // deposit runes
    ctx.deposit_runes_to(
        &[(&foo_rune_id, 100), (&bar_rune_id, 200)],
        &ctx.eth_wallet,
        RuneDepositStrategy::AllInOne,
    )
    .await;

    // check balances
    let ctx = Arc::new(ctx);
    let ctx_t = ctx.clone();

    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            let before_balances = before_balances.clone();
            Box::pin(async move {
                let after_balances = ctx_t
                    .wrapped_balances(&[foo_rune_id, bar_rune_id], &ctx_t.eth_wallet)
                    .await;

                if after_balances[&foo_rune_id] == before_balances[&foo_rune_id] + 100
                    && after_balances[&bar_rune_id] == before_balances[&bar_rune_id] + 200
                {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Balances are not correct"))
                }
            })
        },
        Duration::from_secs(30),
    )
    .await;

    ctx.stop().await
}

#[tokio::test]
async fn test_should_deposit_two_runes_in_two_tx() {
    let ctx = RunesContext::new(&[generate_rune_name(), generate_rune_name()]).await;
    let foo_rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    let bar_rune_id = ctx.runes.runes.keys().nth(1).copied().unwrap();

    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let before_balances = ctx
        .wrapped_balances(&[foo_rune_id, bar_rune_id], &ctx.eth_wallet)
        .await;
    // deposit runes
    ctx.deposit_runes_to(
        &[(&foo_rune_id, 100), (&bar_rune_id, 200)],
        &ctx.eth_wallet,
        RuneDepositStrategy::OnePerTx,
    )
    .await;

    // check balances
    let ctx = Arc::new(ctx);
    let ctx_t = ctx.clone();

    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            let before_balances = before_balances.clone();
            Box::pin(async move {
                let after_balances = ctx_t
                    .wrapped_balances(&[foo_rune_id, bar_rune_id], &ctx_t.eth_wallet)
                    .await;

                if after_balances[&foo_rune_id] == before_balances[&foo_rune_id] + 100
                    && after_balances[&bar_rune_id] == before_balances[&bar_rune_id] + 200
                {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Balances are not correct"))
                }
            })
        },
        Duration::from_secs(30),
    )
    .await;

    ctx.stop().await
}

#[tokio::test]
async fn bail_out_of_impossible_deposit() {
    let rune_name = generate_rune_name();
    let ctx = RunesContext::new(&[rune_name.clone()]).await;

    let rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    let rune_name = RuneName::from_str(&rune_name).unwrap();
    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let address = ctx
        .get_deposit_address(&ctx.eth_wallet.address().into())
        .await;
    let address = Address::from_str(&address).unwrap().assume_checked();
    ctx.send_runes(&address, &[(&rune_id, 10_000)]).await;
    ctx.send_deposit_notification(
        &[rune_id],
        &ctx.eth_wallet.address().into(),
        Some([(rune_name, 5000)].into()),
    )
    .await;

    ctx.inner.advance_time(Duration::from_secs(10)).await;
    ctx.inner.advance_by_times(Duration::from_secs(5), 3).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let client = std::sync::Arc::new(ctx.inner.rune_bridge_client(ADMIN));
    let address = ctx.eth_wallet.address();

    let operations = block_until_succeeds(
        move || {
            let client = client.clone();
            Box::pin(async move {
                let operations = client
                    .get_operations_list(&address.into(), None, None)
                    .await?;

                if operations.len() == 1 {
                    Ok(operations)
                } else {
                    Err(anyhow::anyhow!(
                        "Expected 1 operation, got {}",
                        operations.len()
                    ))
                }
            })
        },
        Duration::from_secs(30),
    )
    .await;

    let client = ctx.inner.rune_bridge_client(ADMIN);
    let operation_id = operations[0].0;

    let log = client
        .get_operation_log(operation_id)
        .await
        .unwrap()
        .unwrap();

    let len = log.log().len();
    // First entry in the log is the scheduling of the operation, so we skip it. There might be other
    // errors, but none of them should be a `cannot progress` error, so we check it here.
    for entry in log.log().iter().take(len.saturating_sub(1)).skip(1) {
        assert!(!entry
            .step_result
            .clone()
            .unwrap_err()
            .to_string()
            .contains("operation cannot progress"));
    }

    assert!(log
        .log()
        .last()
        .unwrap()
        .step_result
        .clone()
        .unwrap_err()
        .to_string()
        .contains("operation cannot progress"));

    ctx.stop().await
}
