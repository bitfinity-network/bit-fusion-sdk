use std::collections::HashSet;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use alloy_sol_types::SolCall;
use bitcoin::key::Secp256k1;
use bitcoin::{Address, Amount, PrivateKey, Txid};
use brc20_bridge::interface::{DepositError, GetAddressError};
use brc20_bridge::ops::Brc20DepositRequestData;
use bridge_client::BridgeCanisterClient;
use bridge_did::brc20_info::Brc20Tick;
use bridge_did::event_data::MinterNotificationType;
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{Brc20BridgeDepositOp, Brc20BridgeOp};
use bridge_utils::BFTBridge;
use candid::{Encode, Principal};
use dashmap::DashMap;
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::{TransactionReceipt, H160, H256, U256};
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ord_rs::Utxo;
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::context::{CanisterType, TestContext};
use crate::dfx_tests::{DfxTestContext, ADMIN};
use crate::utils::brc20_helper::Brc20Helper;
use crate::utils::btc_rpc_client::BitcoinRpcClient;
use crate::utils::hiro_ordinals_client::HiroOrdinalsClient;
use crate::utils::miner::{Exit, Miner};
use crate::utils::token_amount::TokenAmount;

/// Maximum supply of the BRC20 token
pub const DEFAULT_MAX_AMOUNT: u64 = 21_000_000;
/// Initial supply of the BRC20 token for the wallet
pub const DEFAULT_MINT_AMOUNT: u64 = 100_000;
/// Required confirmations for the deposit
pub const REQUIRED_CONFIRMATIONS: u64 = 6;

#[derive(Debug, Clone, Copy)]
pub struct Brc20InitArgs {
    pub tick: Brc20Tick,
    pub decimals: Option<u8>,
    pub limit: Option<u64>,
    pub max_supply: u64,
}

pub struct Brc20Wallet {
    pub admin_address: Address,
    pub admin_btc_rpc_client: Arc<BitcoinRpcClient>,
    pub ord_wallet: BtcWallet,
    pub brc20_tokens: HashSet<Brc20Tick>,
}

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

pub fn generate_brc20_tick() -> Brc20Tick {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let mut name = String::new();
    for _ in 0..4 {
        name.push(rng.gen_range(b'a'..=b'z') as char);
    }
    Brc20Tick::from_str(&name).unwrap()
}

pub fn generate_wallet_name() -> String {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let mut name = String::new();
    for _ in 0..12 {
        name.push(rng.gen_range(b'a'..=b'z') as char);
    }

    name
}

pub struct Brc20Context<Ctx>
where
    Ctx: TestContext + Sync,
{
    pub inner: Ctx,
    pub eth_wallet: Wallet<'static, SigningKey>,
    pub bft_bridge_contract: Arc<RwLock<H160>>,
    exit: Exit,
    miner: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub brc20: Brc20Wallet,
    pub tokens: DashMap<Brc20Tick, H160>,
}

/// Setup a new brc20 for DFX tests
async fn dfx_brc20_setup(brc20_to_deploy: &[Brc20InitArgs]) -> anyhow::Result<Brc20Wallet> {
    let wallet_name = generate_wallet_name();
    let admin_btc_rpc_client = BitcoinRpcClient::dfx_test_client(&wallet_name);
    let admin_address = admin_btc_rpc_client.get_new_address()?;

    println!("Dfx BTC wallet address: {}", admin_address);

    //admin_btc_rpc_client.generate_to_address(&admin_address, 101)?;

    // create ord wallet
    let ord_wallet = generate_btc_wallet();

    let mut brc20_tokens = HashSet::new();

    let brc20_helper = Brc20Helper::new(
        &admin_btc_rpc_client,
        &ord_wallet.private_key,
        &ord_wallet.address,
    );

    for brc20 in brc20_to_deploy {
        let Brc20InitArgs {
            tick,
            decimals,
            limit,
            max_supply,
        } = *brc20;

        // deploy
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

        let deploy_utxo =
            admin_btc_rpc_client.get_utxo_by_address(&commit_fund_tx, &ord_wallet.address)?;

        let deploy_reveal_txid = brc20_helper
            .deploy(
                tick,
                max_supply,
                limit,
                decimals.map(|d| d as u64),
                deploy_utxo,
            )
            .await?;
        println!("BRC20 deploy txid: {}", deploy_reveal_txid);
        brc20_helper
            .wait_for_confirmations(&deploy_reveal_txid, 6)
            .await?;

        // mint
        let commit_fund_tx =
            admin_btc_rpc_client.send_to_address(&ord_wallet.address, Amount::from_int_btc(1))?;
        admin_btc_rpc_client.generate_to_address(&admin_address, 1)?;

        let mint_utxo =
            admin_btc_rpc_client.get_utxo_by_address(&commit_fund_tx, &ord_wallet.address)?;
        let mint_reveal_txid = brc20_helper
            .mint(tick, limit.unwrap_or(DEFAULT_MINT_AMOUNT), mint_utxo)
            .await?;

        println!("BRC20 mint txid: {}", mint_reveal_txid);
        brc20_helper
            .wait_for_confirmations(&mint_reveal_txid, 6)
            .await?;

        brc20_tokens.insert(tick);
    }

    Ok(Brc20Wallet {
        brc20_tokens,
        admin_btc_rpc_client: Arc::new(admin_btc_rpc_client),
        admin_address,
        ord_wallet,
    })
}

impl Brc20Context<DfxTestContext> {
    /// Init BRC20 context for DFX tests
    pub async fn dfx(brc20_to_deploy: &[Brc20InitArgs]) -> Self {
        let context = DfxTestContext::new(&CanisterType::BRC20_CANISTER_SET).await;

        Self::new(context, brc20_to_deploy).await
    }
}

impl<Ctx> Brc20Context<Ctx>
where
    Ctx: TestContext + Sync,
{
    pub async fn new(context: Ctx, brc20_to_deploy: &[Brc20InitArgs]) -> Self {
        let brc20_wallet = dfx_brc20_setup(brc20_to_deploy)
            .await
            .expect("failed to setup brc20 tokens");

        context
            .evm_client(ADMIN)
            .set_logger_filter("info")
            .await
            .expect("failed to set logger filter")
            .unwrap();

        let bridge = context.canisters().brc20_bridge();

        let _: () = context
            .client(bridge, ADMIN)
            .update("admin_configure_ecdsa", ())
            .await
            .unwrap();

        let wallet = context.new_wallet(u128::MAX).await.unwrap();

        let btc_bridge_eth_address = context
            .brc20_bridge_client(ADMIN)
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

        let bft_bridge = context
            .initialize_bft_bridge_with_minter(
                &wallet,
                btc_bridge_eth_address.unwrap(),
                None,
                wrapped_token_deployer,
                true,
            )
            .await
            .unwrap();

        let tokens = DashMap::new();

        for brc20_token in &brc20_wallet.brc20_tokens {
            let token = context
                .create_wrapped_token(&wallet, &bft_bridge, (*brc20_token).into())
                .await
                .unwrap();

            println!("Token {brc20_token} deployed at {token}");
            tokens.insert(*brc20_token, token);
        }

        let _: () = context
            .brc20_bridge_client(ADMIN)
            .set_bft_bridge_contract(&bft_bridge)
            .await
            .unwrap();

        let exit = Exit::new(AtomicBool::new(false));
        let miner = Miner::run(
            brc20_wallet.admin_address.clone(),
            &brc20_wallet.admin_btc_rpc_client,
            &exit,
        );

        Self {
            bft_bridge_contract: Arc::new(RwLock::new(bft_bridge)),
            eth_wallet: wallet,
            exit,
            miner: Arc::new(Mutex::new(Some(miner))),
            inner: context,
            brc20: brc20_wallet,
            tokens,
        }
    }

    pub async fn stop(&self) {
        self.inner
            .stop_canister(self.inner.canisters().evm())
            .await
            .expect("Failed to stop evm canister");
        self.inner
            .stop_canister(self.inner.canisters().brc20_bridge())
            .await
            .expect("Failed to stop brc20 bridge canister");

        self.exit.store(true, std::sync::atomic::Ordering::Relaxed);
        // stop miner
        {
            let mut miner = self.miner.lock().await;
            if let Some(miner) = miner.take() {
                miner.join().expect("Failed to join miner thread");
            }
        }
    }

    pub async fn set_bft_bridge_contract(&self, bft_bridge: &H160) -> anyhow::Result<()> {
        self.inner
            .brc20_bridge_client(ADMIN)
            .set_bft_bridge_contract(bft_bridge)
            .await?;
        println!("BFT bridge contract updated to {bft_bridge}");

        *self.bft_bridge_contract.write().unwrap() = bft_bridge.clone();

        // clear tokens
        self.tokens.clear();

        Ok(())
    }

    pub async fn create_wrapped_token(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        tick: Brc20Tick,
    ) -> anyhow::Result<H160> {
        let bft_bridge_contract = self.bft_bridge_contract.read().unwrap().clone();

        let token = self
            .inner
            .create_wrapped_token(wallet, &bft_bridge_contract, tick.into())
            .await?;

        self.tokens.insert(tick, token.clone());

        Ok(token)
    }

    pub async fn get_funding_utxo(&self, to: &Address) -> anyhow::Result<Utxo> {
        let fund_tx = self
            .brc20
            .admin_btc_rpc_client
            .send_to_address(to, Amount::from_int_btc(1))?;
        self.brc20.admin_btc_rpc_client.generate_to_address(to, 1)?;

        let utxo = self
            .brc20
            .admin_btc_rpc_client
            .get_utxo_by_address(&fund_tx, to)?;

        Ok(utxo)
    }

    pub fn brc20_wallet_address(&self) -> &Address {
        &self.brc20.ord_wallet.address
    }

    pub fn bridge(&self) -> Principal {
        self.inner.canisters().brc20_bridge()
    }

    pub async fn get_deposit_address(&self, eth_address: &H160) -> Address {
        self.inner
            .client(self.bridge(), ADMIN)
            .query::<_, Result<String, GetAddressError>>("get_deposit_address", (eth_address,))
            .await
            .expect("canister call failed")
            .map(|addr| Address::from_str(&addr).unwrap().assume_checked())
            .expect("get_deposit_address error")
    }

    pub async fn send_brc20(
        &self,
        from: &BtcWallet,
        recipient: &Address,
        tick: Brc20Tick,
        amount: TokenAmount,
    ) -> anyhow::Result<Txid> {
        let brc20_helper = Brc20Helper::new(
            &self.brc20.admin_btc_rpc_client,
            &from.private_key,
            &from.address,
        );

        println!(
            "Sending {amount} of {tick} from {sender} to {recipient}",
            amount = amount.as_int(),
            sender = from.address
        );

        let inscription_utxo = self.get_funding_utxo(&from.address).await?;
        println!("Inscription utxo: {:?}", inscription_utxo);
        let transfer_utxo = self.get_funding_utxo(&from.address).await?;
        println!("Transfer utxo: {:?}", transfer_utxo);

        let transfer_txid = brc20_helper
            .transfer(
                tick,
                amount.as_int() as u64,
                recipient.clone(),
                inscription_utxo,
                transfer_utxo,
            )
            .await?;

        println!("BRC20 transfer txid: {}", transfer_txid);

        Ok(transfer_txid)
    }

    /// Wait for the specified number of blocks to be mined
    pub async fn wait_for_blocks(&self, count: u64) {
        let block_height = self
            .brc20
            .admin_btc_rpc_client
            .get_block_height()
            .expect("failed to get block count");
        let target = block_height + count;

        while self
            .brc20
            .admin_btc_rpc_client
            .get_block_height()
            .expect("failed to get block count")
            < target
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Deposit the specified amount to the BRC20 bridge to the provided address
    ///
    /// ## Arguments
    ///
    /// - `tick` - BRC20 token tick
    /// - `amount` - amount to deposit
    /// - `dst_address` - address of the wallet that will receive the tokens
    /// - `sender` - wallet that will sign the transaction
    /// - `nonce` - nonce of the transaction
    pub async fn deposit(
        &self,
        tick: Brc20Tick,
        amount: TokenAmount,
        dst_address: &H160,
        sender: &Wallet<'static, SigningKey>,
        nonce: U256,
        memo: Option<[u8; 32]>,
    ) -> Result<(), DepositError> {
        let dst_token = self.tokens.get(&tick).expect("token not found").clone();

        let client = self.inner.evm_client(ADMIN);
        let chain_id = client.eth_chain_id().await.expect("failed to get chain id");

        let data = Brc20DepositRequestData {
            dst_address: dst_address.clone(),
            dst_token,
            amount: amount.amount(),
            brc20_tick: tick,
        };

        let input = BFTBridge::notifyMinterCall {
            notificationType: MinterNotificationType::DepositRequest as u32,
            userData: Encode!(&data).unwrap().into(),
            memo: memo
                .map(|memo| memo.into())
                .unwrap_or(alloy_sol_types::private::FixedBytes::ZERO),
        }
        .abi_encode();

        let bft_bridge_contract = self.bft_bridge_contract.read().unwrap().clone();

        let transaction = TransactionBuilder {
            from: &sender.address().into(),
            to: Some(bft_bridge_contract),
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
        println!(
            "Deposit notification sent by tx: 0x{}",
            hex::encode(tx_id.0)
        );

        // mint blocks required for confirmations
        self.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;
        const MAX_WAIT: Duration = Duration::from_secs(60);
        const OP_INTERVAL: Duration = Duration::from_secs(5);
        let start = Instant::now();

        while start.elapsed() < MAX_WAIT {
            println!(
                "Checking deposit status. Elapsed {}s...",
                start.elapsed().as_secs()
            );

            let response: Vec<(OperationId, Brc20BridgeOp)> = self
                .inner
                .brc20_bridge_client(ADMIN)
                .get_operations_list(dst_address, None, None)
                .await
                .expect("canister call failed");

            if !response.is_empty() {
                for (_, op) in &response {
                    if matches!(
                        op,
                        Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { .. })
                    ) {
                        return Ok(());
                    }
                }
            }

            println!("Deposit response: {response:?}");
            self.inner.advance_time(OP_INTERVAL).await;
        }

        Err(DepositError::NothingToDeposit)
    }

    /// Withdraw to the specified recipient
    pub async fn withdraw(
        &self,
        recipient: &Address,
        tick: &Brc20Tick,
        amount: TokenAmount,
    ) -> anyhow::Result<()> {
        let token_address = self
            .tokens
            .get(tick)
            .expect("token not found")
            .value()
            .clone();

        println!("Burning {amount} of {tick} to {recipient}");
        let bft_bridge_contract = self.bft_bridge_contract.read().unwrap().clone();

        let client = self.inner.evm_client(ADMIN);
        self.inner
            .burn_erc_20_tokens_raw(
                &client,
                &self.eth_wallet,
                &token_address,
                Id256::from(*tick).0.as_slice(),
                recipient.to_string().as_bytes().to_vec(),
                &bft_bridge_contract,
                amount.amount(),
                true,
                None,
            )
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Failed to burn tokens: {e}"))
    }

    /// Mint tokens to admin wallet and then transfer to the recipient
    pub async fn mint(
        &self,
        tick: Brc20Tick,
        amount: TokenAmount,
        recipient: &Address,
    ) -> anyhow::Result<Txid> {
        let commit_fund_tx = self
            .brc20
            .admin_btc_rpc_client
            .send_to_address(&self.brc20.ord_wallet.address, Amount::from_sat(10_000_000))?;

        let mint_utxo = self
            .brc20
            .admin_btc_rpc_client
            .get_utxo_by_address(&commit_fund_tx, &self.brc20.ord_wallet.address)?;

        let brc20_helper = Brc20Helper::new(
            &self.brc20.admin_btc_rpc_client,
            &self.brc20.ord_wallet.private_key,
            &self.brc20.ord_wallet.address,
        );

        let mint_reveal_txid = brc20_helper
            .mint(tick, amount.as_int() as u64, mint_utxo)
            .await?;

        // mint blocks required for confirmations\
        brc20_helper
            .wait_for_confirmations(&mint_reveal_txid, REQUIRED_CONFIRMATIONS as u32)
            .await?;

        let inscription_utxo = self
            .get_funding_utxo(&self.brc20.ord_wallet.address)
            .await?;
        println!("Inscription utxo: {:?}", inscription_utxo);
        let transfer_utxo = self
            .get_funding_utxo(&self.brc20.ord_wallet.address)
            .await?;
        println!("Transfer utxo: {:?}", transfer_utxo);

        // transfer to the recipient
        let transfer_txid = brc20_helper
            .transfer(
                tick,
                amount.as_int() as u64,
                recipient.clone(),
                inscription_utxo,
                transfer_utxo,
            )
            .await?;

        brc20_helper
            .wait_for_confirmations(&transfer_txid, REQUIRED_CONFIRMATIONS as u32)
            .await?;

        Ok(transfer_txid)
    }

    pub async fn send_btc(&self, btc_address: &Address, amount: Amount) -> anyhow::Result<()> {
        let txid = self
            .brc20
            .admin_btc_rpc_client
            .send_to_address(btc_address, amount)
            .expect("failed to send btc");

        let brc20_helper = Brc20Helper::new(
            &self.brc20.admin_btc_rpc_client,
            &self.brc20.ord_wallet.private_key,
            &self.brc20.ord_wallet.address,
        );
        brc20_helper.wait_for_confirmations(&txid, 6).await
    }

    pub async fn wait_for_tx_success(&self, tx_hash: &H256) -> TransactionReceipt {
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
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        panic!("Transaction {tx_hash} timed out");
    }

    pub async fn wrapped_balance(&self, tick: &Brc20Tick, wallet: &Wallet<'_, SigningKey>) -> u128 {
        let token_contract = self
            .tokens
            .get(tick)
            .expect("token not found")
            .value()
            .clone();

        self.inner
            .check_erc20_balance(&token_contract, wallet, None)
            .await
            .expect("Failed to get wrapped token balance")
    }

    pub async fn brc20_balance(
        &self,
        address: &Address,
        tick: &Brc20Tick,
    ) -> anyhow::Result<TokenAmount> {
        let client = HiroOrdinalsClient::dfx_test_client();

        let balances = client.get_brc20_balances(address).await?;

        let amount = balances
            .get(tick)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("balance not found"))?;

        Ok(amount)
    }
}
