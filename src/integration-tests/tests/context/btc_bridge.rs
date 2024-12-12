use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr as _;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

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
use did::{H160, U256};
use eth_signer::{Signer as _, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ic_ckbtc_kyt::SetApiKeyArg;
use ic_ckbtc_minter::updates::get_btc_address::GetBtcAddressArgs;
use icrc_client::account::Account;
use ord_rs::Utxo;
use tokio::sync::{Mutex, RwLock};

use super::{CanisterType, TestContext};
use crate::utils::btc_rpc_client::BitcoinRpcClient;
use crate::utils::miner::{Exit, Miner};

pub const REQUIRED_CONFIRMATIONS: u64 = 6;

pub struct BtcContext<Ctx>
where
    Ctx: TestContext + Sync,
{
    admin_btc_rpc_client: Arc<BitcoinRpcClient>,
    admin_address: Address,
    pub context: Ctx,
    pub tip_height: AtomicU32,
    pub token_id: Id256,
    exit: Exit,
    miner: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub btf_bridge_contract: Arc<RwLock<H160>>,
    pub wrapped_token: H160,
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
        let context = crate::pocket_ic_integration_test::PocketIcTestContext::new_with(
            &CanisterType::BTC_CANISTER_SET,
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
        )
        .await
        .live();

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

        let btc_bridge_eth_address = context
            .btc_bridge_client(context.admin_name())
            .get_bridge_canister_evm_address()
            .await
            .expect("failed to get btc bridge eth address");

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
            .admin_mint_native_tokens(btc_bridge_eth_address.clone().unwrap(), u64::MAX.into())
            .await
            .expect("failed to mint tokens")
            .expect("failed to mint tokens");

        context.advance_time(Duration::from_secs(2)).await;

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

        let token_id = Id256::from(&context.canisters().ckbtc_ledger());
        let token = context
            .create_wrapped_token(&wallet, &btf_bridge, token_id)
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
            wrapped_token: token,
            exit,
            miner: Arc::new(Mutex::new(Some(miner))),
            token_id,
            tip_height: AtomicU32::default(),
            btf_bridge_contract: Arc::new(RwLock::new(btf_bridge)),
        }
    }

    fn bridge(&self) -> Principal {
        self.context.canisters().btc_bridge()
    }

    pub async fn btc_to_erc20(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        eth_address: &H160,
    ) -> anyhow::Result<()> {
        let user_data = BtcDeposit {
            recipient: eth_address.clone(),
            approve_after_mint: Some(ApproveAfterMint {
                approve_spender: wallet.address().into(),
                approve_amount: U256::from(1000_u64),
            }),
            fee_payer: Some(eth_address.clone()),
        };
        let encoded_reason = Encode!(&user_data).unwrap();

        let input = BTFBridge::notifyMinterCall {
            notificationType: Default::default(),
            userData: encoded_reason.into(),
            memo: alloy_sol_types::private::FixedBytes::ZERO,
        }
        .abi_encode();

        // advance
        self.context.advance_time(Duration::from_secs(2)).await;

        let receipt = self
            .context
            .call_contract(
                wallet,
                &self.btf_bridge_contract.read().await.clone(),
                input,
                0,
            )
            .await
            .map(|(_, receipt)| receipt)?;

        println!("btc_to_erc20 receipt: {:?}", receipt);

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

    pub async fn mint_wrapped_btc(
        &self,
        amount: u64,
        wallet: &Wallet<'_, SigningKey>,
    ) -> anyhow::Result<()> {
        let caller_eth_address = wallet.address().0.into();

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

        self.btc_to_erc20(wallet, &caller_eth_address).await
    }

    pub async fn erc20_balance_of(&self, wallet: &Wallet<'_, SigningKey>) -> anyhow::Result<u128> {
        self.context
            .check_erc20_balance(&self.wrapped_token, wallet, None)
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
            .stop_canister(self.context.canisters().bitcoin())
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
