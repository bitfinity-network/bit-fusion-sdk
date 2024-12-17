use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

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
use did::{TransactionReceipt, H160, H256, U256};
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ord_rs::Utxo;
use ordinals::{Etching, Rune, RuneId, Terms};
use rune_bridge::interface::{DepositError, GetAddressError};
use rune_bridge::ops::RuneDepositRequestData;
use tokio::sync::{Mutex, RwLock};
use tokio::time::Instant;

use super::CanisterType;
use crate::context::TestContext;
use crate::utils::btc_rpc_client::BitcoinRpcClient;
use crate::utils::miner::{Exit, Miner};
use crate::utils::ord_client::OrdClient;
use crate::utils::rune_helper::RuneHelper;

type AsyncMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

pub const REQUIRED_CONFIRMATIONS: u64 = 6;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RuneDepositStrategy {
    AllInOne,
    OnePerTx,
}

pub struct RuneWalletInfo {
    pub id256: Id256,
    pub name: String,
}

pub struct RuneWallet {
    pub admin_address: Address,
    pub admin_btc_rpc_client: Arc<BitcoinRpcClient>,
    pub ord_wallet: BtcWallet,
    pub runes: HashMap<RuneId, RuneWalletInfo>,
}

pub struct RunesContext<Ctx>
where
    Ctx: TestContext + Sync,
{
    pub inner: Ctx,
    pub eth_wallet: Wallet<'static, SigningKey>,
    pub btf_bridge_contract: Arc<RwLock<H160>>,
    exit: Exit,
    miner: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub runes: RuneWallet,
    pub tokens: AsyncMap<RuneId, H160>,
}

pub fn generate_rune_name() -> String {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let mut name = String::new();
    for _ in 0..16 {
        name.push(rng.gen_range(b'A'..=b'Z') as char);
    }
    name
}

/// Setup new runes for tests.
///
/// Creates a new wallet for the admin, and etches the specified runes.
async fn rune_setup(runes_to_etch: &[String]) -> anyhow::Result<RuneWallet> {
    let rune_name = generate_rune_name();
    let admin_btc_rpc_client = BitcoinRpcClient::test_client(&rune_name);
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

#[cfg(feature = "pocket_ic_integration_test")]
impl RunesContext<crate::pocket_ic_integration_test::PocketIcTestContext> {
    /// Init Rune context for [`PocketIcTestContext`] to run on pocket-ic
    pub async fn pocket_ic(runes: &[String]) -> Self {
        let context = crate::pocket_ic_integration_test::PocketIcTestContext::new_with(
            &CanisterType::RUNE_CANISTER_SET,
            |builder| {
                builder
                    .with_ii_subnet()
                    .with_bitcoin_subnet()
                    .with_bitcoind_addr(SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        18444,
                    ))
            },
            |mut pic| {
                Box::pin(async move {
                    // NOTE: set time: Because the bitcoind process uses the real time, we set the time of the PocketIC instance to be the current time:
                    pic.set_time(SystemTime::now()).await;
                    pic.make_live(None).await;
                    pic
                })
            },
            true,
        )
        .await;

        Self::new(context, runes).await
    }
}

impl<Ctx> RunesContext<Ctx>
where
    Ctx: TestContext + Sync,
{
    pub async fn new(context: Ctx, runes: &[String]) -> Self {
        let rune_wallet = rune_setup(runes).await.expect("failed to setup runes");

        context
            .evm_client(context.admin_name())
            .set_logger_filter("info")
            .await
            .expect("failed to set logger filter")
            .unwrap();

        let bridge = context.canisters().rune_bridge();

        let _: () = context
            .client(bridge, context.admin_name())
            .update("admin_configure_ecdsa", ())
            .await
            .unwrap();

        context.advance_time(Duration::from_secs(10)).await;

        let rune_bridge_eth_address = context
            .rune_bridge_client(context.admin_name())
            .get_bridge_canister_evm_address()
            .await
            .unwrap();

        let mut rng = rand::thread_rng();
        let wallet = Wallet::new(&mut rng);
        let wallet_address = wallet.address();

        context
            .evm_client(context.admin_name())
            .admin_mint_native_tokens(wallet_address.into(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();

        let client = context.evm_client(context.admin_name());
        client
            .admin_mint_native_tokens(rune_bridge_eth_address.clone().unwrap(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();

        context.advance_time(Duration::from_secs(2)).await;

        let wrapped_token_deployer = context
            .initialize_wrapped_token_deployer_contract(&wallet)
            .await
            .unwrap();
        let btf_bridge = context
            .initialize_btf_bridge_with_minter(
                &wallet,
                rune_bridge_eth_address.unwrap(),
                None,
                wrapped_token_deployer,
                true,
            )
            .await
            .unwrap();

        let tokens = AsyncMap::default();

        for rune_id in rune_wallet.runes.keys() {
            let token = context
                .create_wrapped_token(&wallet, &btf_bridge, (*rune_id).into())
                .await
                .unwrap();

            tokens.write().await.insert(*rune_id, token);
        }

        let _: () = context
            .rune_bridge_client(context.admin_name())
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
            btf_bridge_contract: Arc::new(RwLock::new(btf_bridge)),
            eth_wallet: wallet,
            exit,
            miner: Arc::new(Mutex::new(Some(miner))),
            inner: context,
            runes: rune_wallet,
            tokens,
        }
    }

    fn bridge(&self) -> Principal {
        self.inner.canisters().rune_bridge()
    }

    pub async fn get_deposit_address(&self, eth_address: &H160) -> Address {
        self.inner
            .client(self.bridge(), self.inner.admin_name())
            .query::<_, Result<String, GetAddressError>>("get_deposit_address", (eth_address,))
            .await
            .expect("canister call failed")
            .map(|addr| Address::from_str(&addr).unwrap().assume_checked())
            .expect("get_deposit_address error")
    }

    pub async fn send_runes(
        &self,
        from: &BtcWallet,
        btc_address: &Address,
        runes: &[(&RuneId, u128)],
    ) -> anyhow::Result<()> {
        let etcher = RuneHelper::new(
            &self.runes.admin_btc_rpc_client,
            &from.private_key,
            &from.address,
        );

        // load utxos
        let mut utxos = Vec::with_capacity(runes.len());
        for (rune_id, _) in runes {
            let rune_info = self
                .runes
                .runes
                .get(rune_id)
                .ok_or_else(|| anyhow::anyhow!("Rune not found"))?;

            // find the utxo
            let balance = OrdClient::test_client()
                .get_balances(&rune_info.name)
                .await?;

            for outpoint in balance.keys() {
                let outpoint_info = OrdClient::test_client().get_outpoint(outpoint).await?;

                let tokens = outpoint.split(':').collect::<Vec<_>>();
                let txid = Txid::from_str(tokens[0]).expect("failed to parse txid");
                let index = tokens[1].parse::<u32>().expect("failed to parse index");

                if outpoint_info.address == from.address {
                    utxos.push(Utxo {
                        index,
                        id: txid,
                        amount: outpoint_info.value,
                    });
                }
            }
        }

        if utxos.len() < runes.len() {
            anyhow::bail!("Runes not found; got {utxos:?}; required {runes:?}");
        }

        // get funding utxo
        let edict_fund_tx = self
            .send_btc(&from.address, Amount::from_sat(10_000_000))
            .await?;

        let edict_funds_utxo = self
            .runes
            .admin_btc_rpc_client
            .get_utxo_by_address(&edict_fund_tx, &from.address)?;

        let mut inputs = utxos;
        inputs.push(edict_funds_utxo);

        let amounts = runes;
        let runes = runes
            .iter()
            .map(|(rune_id, amount)| (**rune_id, *amount))
            .collect::<Vec<_>>();

        let tx_id = etcher
            .edict_runes(inputs, runes, btc_address.clone())
            .await?;

        self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;
        println!(
            "{runes_count} Runes sent. txid: {tx_id}; sent to {btc_address}; amounts: {amounts:?}",
            runes_count = amounts.len(),
        );

        Ok(())
    }

    pub async fn send_btc(&self, btc_address: &Address, amount: Amount) -> anyhow::Result<Txid> {
        let txid = self
            .runes
            .admin_btc_rpc_client
            .send_to_address(btc_address, amount)
            .expect("failed to send btc");

        self.wait_for_confirmations(&txid, REQUIRED_CONFIRMATIONS)
            .await?;

        Ok(txid)
    }

    async fn wait_for_confirmations(
        &self,
        txid: &Txid,
        required_confirmations: u64,
    ) -> anyhow::Result<()> {
        // ! let's wait for 6 confirmations - ord won't index under 6 confirmations
        let start = Instant::now();
        loop {
            self.runes
                .admin_btc_rpc_client
                .generate_to_address(&self.runes.admin_address, 1)?;
            let confirmations: u32 = self
                .runes
                .admin_btc_rpc_client
                .get_transaction_confirmations(txid)?;
            println!("commit transaction {txid} confirmations: {}", confirmations);
            if confirmations >= required_confirmations as u32 {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                anyhow::bail!("commit transaction not confirmed after 60 seconds");
            }
        }

        Ok(())
    }

    /// Wait for the specified number of blocks to be mined
    pub async fn wait_for_blocks(&self, count: u64) {
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
            self.inner.advance_time(Duration::from_millis(100)).await;
        }
    }

    pub async fn deposit(
        &self,
        runes: &[RuneId],
        amounts: Option<HashMap<RuneName, u128>>,
        dst_address: &H160,
        sender: &Wallet<'static, SigningKey>,
        nonce: U256,
        memo: Option<[u8; 32]>,
    ) -> Result<(), DepositError> {
        self.send_deposit_notification(runes, amounts, dst_address, sender, nonce, memo)
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
                .rune_bridge_client(self.inner.admin_name())
                .get_operations_list(dst_address, None, None)
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

    pub async fn send_deposit_notification(
        &self,
        runes: &[RuneId],
        amounts: Option<HashMap<RuneName, u128>>,
        dst_address: &H160,
        sender: &Wallet<'static, SigningKey>,
        nonce: U256,
        memo: Option<[u8; 32]>,
    ) {
        let mut dst_tokens = HashMap::new();
        for rune_id in runes {
            let erc20_address = self
                .tokens
                .read()
                .await
                .get(rune_id)
                .expect("token not found")
                .clone();
            let rune_info = self.runes.runes.get(rune_id).expect("rune not found");

            dst_tokens.insert(RuneName::from_str(&rune_info.name).unwrap(), erc20_address);
        }

        let client = self.inner.evm_client(self.inner.admin_name());
        let chain_id = client.eth_chain_id().await.expect("failed to get chain id");

        let data = RuneDepositRequestData {
            dst_address: dst_address.clone(),
            dst_tokens,
            amounts,
        };

        let input = BTFBridge::notifyMinterCall {
            notificationType: MinterNotificationType::DepositRequest as u32,
            userData: Encode!(&data).unwrap().into(),
            memo: memo
                .map(|memo| memo.into())
                .unwrap_or(alloy_sol_types::private::FixedBytes::ZERO),
        }
        .abi_encode();

        let btf_bridge_contract = self.btf_bridge_contract.read().await.clone();

        let transaction = TransactionBuilder {
            from: &sender.address().into(),
            to: Some(btf_bridge_contract),
            nonce,
            value: Default::default(),
            gas: 5_000_000u64.into(),
            gas_price: Some((EIP1559_INITIAL_BASE_FEE * 2).into()),
            input,
            signature: SigningMethod::SigningKey(sender.signer()),
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
        let client = self.inner.evm_client(self.inner.admin_name());
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
                self.inner.advance_time(Duration::from_millis(500)).await;
            }
        }

        panic!("Transaction {tx_hash} timed out");
    }

    /// Withdraw wrapped tokens to the specified address
    pub async fn withdraw(
        &self,
        recipient: &Address,
        rune_id: &RuneId,
        amount: u128,
    ) -> anyhow::Result<()> {
        let token_address = self
            .tokens
            .read()
            .await
            .get(rune_id)
            .expect("token not found")
            .clone();
        let rune_info = self.runes.runes.get(rune_id).expect("rune not found");

        let btf_bridge_contract = self.btf_bridge_contract.read().await.clone();

        println!("Burning {amount} of {rune_id} to {recipient}");

        let client = self.inner.evm_client(self.inner.admin_name());
        self.inner
            .burn_erc_20_tokens_raw(
                &client,
                &self.eth_wallet,
                &token_address,
                rune_info.id256.0.as_slice(),
                recipient.to_string().as_bytes().to_vec(),
                &btf_bridge_contract,
                amount,
                true,
                None,
            )
            .await?;

        self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;

        Ok(())
    }

    pub async fn set_btf_bridge_contract(&self, btf_bridge: &H160) -> anyhow::Result<()> {
        self.inner
            .rune_bridge_client(self.inner.admin_name())
            .set_btf_bridge_contract(btf_bridge)
            .await?;
        println!("btf bridge contract updated to {btf_bridge}");

        *self.btf_bridge_contract.write().await = btf_bridge.clone();

        // clear tokens
        self.tokens.write().await.clear();

        Ok(())
    }

    pub async fn create_wrapped_token(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        rune: RuneId,
    ) -> anyhow::Result<H160> {
        let btf_bridge_contract = self.btf_bridge_contract.read().await.clone();

        let token = self
            .inner
            .create_wrapped_token(wallet, &btf_bridge_contract, rune.into())
            .await?;

        self.tokens.write().await.insert(rune, token.clone());

        Ok(token)
    }

    pub async fn wrapped_balance(&self, rune_id: &RuneId, wallet: &Wallet<'_, SigningKey>) -> u128 {
        let token_contract = self
            .tokens
            .read()
            .await
            .get(rune_id)
            .expect("token not found")
            .clone();

        self.inner
            .check_erc20_balance(&token_contract, wallet, None)
            .await
            .expect("Failed to get wrapped token balance")
    }

    pub async fn wrapped_balances(
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

    pub async fn ord_rune_balance(
        &self,
        address: &Address,
        rune_id: &RuneId,
    ) -> anyhow::Result<u128> {
        let rune_info = self.runes.runes.get(rune_id).expect("rune not found");

        let balance = OrdClient::test_client()
            .get_balances(&rune_info.name)
            .await?;
        let mut amount = 0;
        for (outpoint, balance) in balance {
            let owner = OrdClient::test_client()
                .get_outpoint(&outpoint)
                .await?
                .address;
            println!("found outpoint {outpoint} with balance {balance} owned by {owner}");
            if &owner == address {
                amount += balance as u128;
            }
        }

        Ok(amount)
    }

    pub async fn deposit_runes_to(
        &self,
        runes: &[(&RuneId, u128)],
        dst_address: &H160,
        sender: &Wallet<'static, SigningKey>,
        nonce: U256,
        memo: Option<[u8; 32]>,
        deposit_strategy: RuneDepositStrategy,
    ) -> anyhow::Result<()> {
        let rune_ids = runes
            .iter()
            .map(|(rune_id, _)| **rune_id)
            .collect::<Vec<_>>();
        let balance_before = self.wrapped_balances(&rune_ids, sender).await;

        let wallet_address = sender.address();
        let btc_address = self.get_deposit_address(&wallet_address.into()).await;
        println!("Wallet address: {wallet_address}; deposit_address {btc_address}");

        match deposit_strategy {
            RuneDepositStrategy::OnePerTx => {
                for rune in runes {
                    self.send_runes(&self.runes.ord_wallet, &btc_address, &[*rune])
                        .await?;
                    self.send_btc(&btc_address, Amount::from_int_btc(1)).await?;
                }
            }
            RuneDepositStrategy::AllInOne => {
                self.send_runes(&self.runes.ord_wallet, &btc_address, runes)
                    .await?;
                self.send_btc(&btc_address, Amount::from_int_btc(1)).await?;
            }
        }

        self.deposit(&rune_ids, None, dst_address, sender, nonce, memo)
            .await
            .expect("failed to deposit runes");

        let balance_after = self.wrapped_balances(&rune_ids, sender).await;

        for (rune_id, rune_amount) in runes {
            let balance_after = balance_after.get(rune_id).copied().ok_or_else(|| {
                anyhow::anyhow!("Wrapped token balance of the wallet not found after")
            })?;
            let balance_before = balance_before.get(rune_id).copied().ok_or_else(|| {
                anyhow::anyhow!("Wrapped token balance of the wallet not found before")
            })?;
            assert_eq!(balance_after - balance_before, *rune_amount, "Wrapped token balance of the wallet changed by unexpected amount. Balance before: {balance_before}, balance_after: {balance_after}, deposit amount: {rune_amount}");
        }

        self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;

        Ok(())
    }

    pub async fn stop(&self) {
        self.inner
            .stop_canister(self.inner.canisters().evm())
            .await
            .expect("Failed to stop evm canister");
        self.inner
            .stop_canister(self.inner.canisters().rune_bridge())
            .await
            .expect("Failed to stop rune bridge canister");
        self.inner
            .stop_canister(self.inner.canisters().bitcoin())
            .await
            .expect("Failed to stop bitcoin canister");

        self.exit.store(true, std::sync::atomic::Ordering::Relaxed);
        // stop miner
        {
            let mut miner = self.miner.lock().await;
            if let Some(miner) = miner.take() {
                miner.join().expect("Failed to join miner thread");
            }
        }
    }
}

#[derive(Clone)]
pub struct BtcWallet {
    pub private_key: PrivateKey,
    pub address: Address,
}

pub fn generate_btc_wallet() -> BtcWallet {
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
