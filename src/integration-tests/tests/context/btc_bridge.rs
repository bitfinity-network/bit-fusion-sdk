use std::str::FromStr as _;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use alloy_sol_types::SolCall;
use bitcoin::{Address, Amount, Txid};
use bridge_client::BridgeCanisterClient as _;
use bridge_did::id256::Id256;
use bridge_did::init::btc::WrappedTokenConfig;
use bridge_did::order::{SignedMintOrder, SignedOrders};
use bridge_did::reason::{ApproveAfterMint, BtcDeposit};
use bridge_utils::BTFBridge;
use btc_bridge::canister::eth_address_to_subaccount;
use candid::{Encode, Nat, Principal};
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::{TransactionReceipt, H160, H256, U256};
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::{Signer as _, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ic_ckbtc_kyt::SetApiKeyArg;
use ic_ckbtc_minter::updates::get_btc_address::GetBtcAddressArgs;
use icrc_client::account::Account;
use ord_rs::Utxo;
use tokio::sync::{Mutex, RwLock};

use super::TestContext;
use crate::utils::btc_rpc_client::BitcoinRpcClient;
use crate::utils::btc_transfer_helper::BtcTransferHelper;
use crate::utils::btc_wallet::BtcWallet;
use crate::utils::miner::{Exit, Miner};

pub const REQUIRED_CONFIRMATIONS: u64 = 6;

pub struct BtcContext<Ctx>
where
    Ctx: TestContext + Sync,
{
    pub admin_btc_rpc_client: Arc<BitcoinRpcClient>,
    pub admin_address: Address,
    pub context: Ctx,
    pub tip_height: AtomicU32,
    pub ckbtc_ledger_id: Id256,
    exit: Exit,
    miner: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub btf_bridge_contract: Arc<RwLock<H160>>,
    pub wrapped_token: Arc<RwLock<H160>>,
}

fn generate_wallet_name() -> String {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let mut name = String::new();
    for _ in 0..16 {
        name.push(rng.gen_range(b'A'..=b'Z') as char);
    }
    name
}

#[cfg(feature = "pocket_ic_integration_test")]
impl BtcContext<crate::pocket_ic_integration_test::PocketIcTestContext> {
    pub async fn pocket_ic() -> Self {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};

        let context = crate::pocket_ic_integration_test::PocketIcTestContext::new_with(
            &crate::context::CanisterType::BTC_CANISTER_SET,
            |builder| {
                builder
                    .with_ii_subnet()
                    .with_bitcoin_subnet()
                    .with_bitcoind_addr(SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        18444,
                    ))
            },
            |pic| {
                Box::pin(async move {
                    // NOTE: set time: Because the bitcoind process uses the real time, we set the time of the PocketIC instance to be the current time:
                    pic.set_time(std::time::SystemTime::now()).await;
                    pic
                })
            },
            false,
        )
        .await;

        Self::new(context).await
    }
}

impl<Ctx> BtcContext<Ctx>
where
    Ctx: TestContext + Sync,
{
    pub async fn new(context: Ctx) -> Self {
        // set KYT api key
        context
            .client(context.canisters().kyt(), context.admin_name())
            .update::<(SetApiKeyArg,), ()>(
                "set_api_key",
                (SetApiKeyArg {
                    api_key: "api key".to_string(),
                },),
            )
            .await
            .expect("failed to set api key");

        let admin_btc_rpc_client = Arc::new(BitcoinRpcClient::test_client(&generate_wallet_name()));
        let admin_address = admin_btc_rpc_client
            .get_new_address()
            .expect("failed to get new address");

        context.advance_time(Duration::from_secs(10)).await;

        let btc_bridge_eth_address = context
            .btc_bridge_client(context.admin_name())
            .get_bridge_canister_evm_address()
            .await
            .expect("failed to get btc bridge eth address")
            .expect("failed to get btc bridge eth address");

        let mut rng = rand::thread_rng();
        let wallet = Wallet::new(&mut rng);
        let wallet_address = wallet.address();

        context
            .evm_client(context.admin_name())
            .admin_mint_native_tokens(wallet_address.into(), u64::MAX.into())
            .await
            .expect("failed to mint tokens to user")
            .expect("failed to mint tokens to user");

        let client = context.evm_client(context.admin_name());
        client
            .admin_mint_native_tokens(btc_bridge_eth_address.clone(), u64::MAX.into())
            .await
            .expect("failed to mint tokens to btc bridge")
            .expect("failed to mint tokens to btc bridge");

        context.advance_time(Duration::from_secs(2)).await;

        let wrapped_token_deployer = context
            .initialize_wrapped_token_deployer_contract(&wallet)
            .await
            .expect("failed to initialize wrapped token deployer");
        let btf_bridge = context
            .initialize_btf_bridge_with_minter(
                &wallet,
                btc_bridge_eth_address,
                None,
                wrapped_token_deployer,
                true,
            )
            .await
            .expect("failed to initialize btf bridge");

        let ckbtc_ledger_id = Id256::from(&context.canisters().ckbtc_ledger());
        let token = context
            .create_wrapped_token(&wallet, &btf_bridge, ckbtc_ledger_id)
            .await
            .expect("failed to create wrapped token");

        println!("wrapped token {token}",);
        let _: () = context
            .btc_bridge_client(context.admin_name())
            .set_btf_bridge_contract(&btf_bridge)
            .await
            .expect("failed to set btf bridge");

        let mut token_name = [0; 32];
        token_name[0..7].copy_from_slice(b"wrapper");
        let mut token_symbol = [0; 16];
        token_symbol[0..3].copy_from_slice(b"WPT");

        let wrapped_token_config = WrappedTokenConfig {
            token_address: token.clone(),
            token_name,
            token_symbol,
            decimals: 0,
        };

        context
            .btc_bridge_client(context.admin_name())
            .admin_configure_wrapped_token(wrapped_token_config)
            .await
            .expect("failed to configure wrapped token")
            .expect("failed to configure wrapped token");

        context.advance_time(Duration::from_secs(2)).await;

        let exit = Arc::new(AtomicBool::new(false));
        let miner = Miner::run(admin_address.clone(), &admin_btc_rpc_client, &exit);

        Self {
            admin_address,
            admin_btc_rpc_client,
            context,
            wrapped_token: Arc::new(RwLock::new(token)),
            exit,
            miner: Arc::new(Mutex::new(Some(miner))),
            ckbtc_ledger_id,
            tip_height: AtomicU32::default(),
            btf_bridge_contract: Arc::new(RwLock::new(btf_bridge)),
        }
    }

    fn bridge(&self) -> Principal {
        self.context.canisters().btc_bridge()
    }

    pub async fn btc_to_erc20(
        &self,
        sender: &Wallet<'_, SigningKey>,
        to: &H160,
        memo: [u8; 32],
        nonce: u64,
    ) -> anyhow::Result<()> {
        let caller_eth_address: H160 = sender.address().0.into();
        println!("btc_to_erc20 caller {caller_eth_address}");
        let user_data = BtcDeposit {
            recipient: to.clone(),
            approve_after_mint: Some(ApproveAfterMint {
                approve_spender: sender.address().into(),
                approve_amount: U256::from(1000_u64),
            }),
            fee_payer: Some(caller_eth_address.clone()),
        };
        let encoded_reason = Encode!(&user_data).unwrap();

        let input = BTFBridge::notifyMinterCall {
            notificationType: Default::default(),
            userData: encoded_reason.into(),
            memo: memo.into(),
        }
        .abi_encode();

        let client = self.context.evm_client(self.context.admin_name());
        let btf_bridge_contract = self.btf_bridge_contract.read().await.clone();
        let chain_id = client.eth_chain_id().await.expect("failed to get chain id");

        let transaction = TransactionBuilder {
            from: &sender.address().into(),
            to: Some(btf_bridge_contract),
            nonce: nonce.into(),
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
        println!(
            "Deposit notification sent by tx: 0x{}",
            hex::encode(tx_id.0)
        );
        self.wait_for_tx_success(&tx_id).await;
        println!("Deposit notification confirmed");

        Ok(())
    }

    pub async fn list_mint_orders(
        &self,
        eth_address: &H160,
    ) -> anyhow::Result<Vec<(u32, SignedMintOrder)>> {
        self.context
            .btc_bridge_client(self.context.admin_name())
            .list_mint_orders(eth_address)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub async fn send_btc(&self, btc_address: &Address, amount: Amount) -> anyhow::Result<Txid> {
        let txid = self
            .admin_btc_rpc_client
            .send_to_address(btc_address, amount)?;

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
            self.admin_btc_rpc_client
                .generate_to_address(&self.admin_address, 1)?;
            let confirmations: u32 = self
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
            .admin_btc_rpc_client
            .get_block_height()
            .expect("failed to get block count");
        let target = block_height + count;
        while self
            .admin_btc_rpc_client
            .get_block_height()
            .expect("failed to get block count")
            < target
        {
            self.context.advance_time(Duration::from_millis(100)).await;
        }
    }

    pub async fn get_mint_order(
        &self,
        eth_address: &H160,
        nonce: u32,
    ) -> anyhow::Result<Option<SignedOrders>> {
        self.context
            .btc_bridge_client(self.context.admin_name())
            .get_mint_order(eth_address, nonce)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Deposit BTC from a BTC wallet
    pub async fn deposit_btc(
        &self,
        from: &Wallet<'_, SigningKey>,
        from_wallet: &BtcWallet,
        amount: Amount,
        to: &H160,
        memo: [u8; 32],
        nonce: u64,
    ) -> anyhow::Result<()> {
        let caller_eth_address = from.address().0.into();

        let deposit_account = Account {
            owner: self.bridge(),
            subaccount: Some(eth_address_to_subaccount(&caller_eth_address).0),
        };

        // get deposit utxo
        let deposit_address = self.get_btc_address(deposit_account).await?;

        // get funding utxo
        let funding_utxo = self
            .get_funding_utxo(&from_wallet.address, amount * 2)
            .await?;

        // send btc to the deposit address
        let txid = BtcTransferHelper::new(
            &self.admin_btc_rpc_client,
            &from_wallet.private_key,
            &from_wallet.address,
        )
        .transfer(amount, funding_utxo, &deposit_address)
        .await?;

        // wait for confirmations
        self.wait_for_confirmations(&txid, REQUIRED_CONFIRMATIONS)
            .await?;

        println!("btc transferred at {txid}");

        self.btc_to_erc20(from, to, memo, nonce).await
    }

    /// Withdraw to the specified recipient
    pub async fn withdraw_btc(
        &self,
        from: &Wallet<'_, SigningKey>,
        recipient: &Address,
        amount: Amount,
    ) -> anyhow::Result<()> {
        let token_address = self.wrapped_token.read().await.clone();

        println!("Burning {amount} to {recipient}");
        let btf_bridge_contract = self.btf_bridge_contract.read().await.clone();

        let client = self.context.evm_client(self.context.admin_name());
        self.context
            .burn_erc_20_tokens_raw(
                &client,
                from,
                &token_address,
                &self.ckbtc_ledger_id.0,
                recipient.to_string().as_bytes().to_vec(),
                &btf_bridge_contract,
                amount.to_sat() as u128,
                true,
                None,
            )
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Failed to burn tokens: {e}"))?;

        Ok(())
    }

    pub async fn mint_admin_wrapped_btc(
        &self,
        amount: u64,
        from: &Wallet<'_, SigningKey>,
        to: &H160,
        nonce: u64,
    ) -> anyhow::Result<()> {
        let caller_eth_address = from.address().0.into();

        let deposit_account = Account {
            owner: self.bridge(),
            subaccount: Some(eth_address_to_subaccount(&caller_eth_address).0),
        };

        // get deposit utxo
        let deposit_address = self.get_btc_address(deposit_account).await?;
        // send utxo to the deposit address
        let funding_utxo = self
            .get_funding_utxo(&deposit_address, Amount::from_sat(amount))
            .await?;
        println!("funding utxo: {:?}", funding_utxo);

        self.btc_to_erc20(from, to, [0; 32], nonce).await
    }

    pub async fn erc20_balance_of(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        address: Option<&H160>,
    ) -> anyhow::Result<u128> {
        let value = self.wrapped_token.read().await.clone();
        self.context
            .check_erc20_balance(&value, wallet, address)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub async fn icrc_balance_of(&self, account: impl Into<Account>) -> anyhow::Result<Nat> {
        let account = account.into();

        let icrc_client = self.context.ckbtc_token_client(self.context.admin_name());
        let balance = icrc_client.icrc1_balance_of(account).await?;

        Ok(balance)
    }

    pub async fn get_btc_address_from_bridge(
        &self,
        account: impl Into<Account>,
    ) -> anyhow::Result<Address> {
        let account = account.into();
        let addr = self
            .context
            .client(
                self.context.canisters().btc_bridge(),
                self.context.admin_name(),
            )
            .update::<(GetBtcAddressArgs,), String>(
                "get_btc_address",
                (GetBtcAddressArgs {
                    owner: Some(account.owner),
                    subaccount: account.subaccount,
                },),
            )
            .await?;

        Ok(Address::from_str(&addr)?.assume_checked())
    }

    pub async fn get_btc_address(&self, account: impl Into<Account>) -> anyhow::Result<Address> {
        let account = account.into();
        let addr = self
            .context
            .client(
                self.context.canisters().ckbtc_minter(),
                self.context.admin_name(),
            )
            .update::<(GetBtcAddressArgs,), String>(
                "get_btc_address",
                (GetBtcAddressArgs {
                    owner: Some(account.owner),
                    subaccount: account.subaccount,
                },),
            )
            .await?;

        Ok(Address::from_str(&addr)?.assume_checked())
    }

    pub async fn set_btf_bridge_contract(&self, btf_bridge: &H160) -> anyhow::Result<()> {
        self.context
            .btc_bridge_client(self.context.admin_name())
            .set_btf_bridge_contract(btf_bridge)
            .await?;
        println!("btf bridge contract updated to {btf_bridge}");

        *self.btf_bridge_contract.write().await = btf_bridge.clone();

        Ok(())
    }

    pub async fn create_wrapped_token(
        &self,
        wallet: &Wallet<'_, SigningKey>,
    ) -> anyhow::Result<H160> {
        let btf_bridge_contract = self.btf_bridge_contract.read().await.clone();

        let token_id = Id256::from(&self.context.canisters().ckbtc_ledger());
        let token = self
            .context
            .create_wrapped_token(wallet, &btf_bridge_contract, token_id)
            .await
            .expect("failed to create wrapped token");

        println!("wrapped token {token}",);

        *self.wrapped_token.write().await = token.clone();

        let mut token_name = [0; 32];
        token_name[0..7].copy_from_slice(b"wrapper");
        let mut token_symbol = [0; 16];
        token_symbol[0..3].copy_from_slice(b"WPT");

        let wrapped_token_config = WrappedTokenConfig {
            token_address: token.clone(),
            token_name,
            token_symbol,
            decimals: 0,
        };

        self.context
            .btc_bridge_client(self.context.admin_name())
            .admin_configure_wrapped_token(wrapped_token_config)
            .await
            .expect("failed to configure wrapped token")
            .expect("failed to configure wrapped token");

        self.context.advance_time(Duration::from_secs(5)).await;

        Ok(token)
    }

    /// Get the BTC balance of the given address
    pub async fn btc_balance(&self, address: &Address) -> Amount {
        self.admin_btc_rpc_client
            .btc_balance(address)
            .expect("failed to get btc balance")
    }

    pub async fn get_funding_utxo(&self, to: &Address, amount: Amount) -> anyhow::Result<Utxo> {
        let fund_tx;
        loop {
            match self.admin_btc_rpc_client.send_to_address(to, amount) {
                Ok(tx) => {
                    fund_tx = tx;
                    break;
                }
                Err(err) => {
                    println!("Failed to send btc: {err}");
                    self.admin_btc_rpc_client
                        .generate_to_address(&self.admin_address, 1)?;
                }
            }
        }

        let utxo = self
            .admin_btc_rpc_client
            .get_utxo_by_address(&fund_tx, to)?;

        Ok(utxo)
    }

    pub async fn stop(&self) {
        self.context
            .stop_canister(self.context.canisters().evm())
            .await
            .expect("Failed to stop evm canister");
        self.context
            .stop_canister(self.bridge())
            .await
            .expect("Failed to stop btc bridge canister");
        self.context
            .stop_canister(
                self.context
                    .canisters()
                    .bitcoin(self.context.is_pocket_ic()),
            )
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

    /// Wait for the transaction to be confirmed by the EVM within a reasonable time frame
    pub async fn wait_for_tx_success(&self, tx_hash: &H256) -> TransactionReceipt {
        const MAX_TX_TIMEOUT_SEC: Duration = Duration::from_secs(10);

        let start = Instant::now();

        let client = self.context.evm_client(self.context.admin_name());
        while start.elapsed() < MAX_TX_TIMEOUT_SEC {
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
                self.context.advance_time(Duration::from_millis(100)).await;
            }
        }

        panic!("Transaction {tx_hash} timed out");
    }
}
