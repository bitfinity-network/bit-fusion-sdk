use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use did::error::{EvmError, TransactionPoolError};
use did::{BlockNumber, Bytes, Transaction, TransactionReceipt, H160, H256, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::transaction::eip2718::TypedTransaction;
use evm_canister_client::{
    CanisterClientError, CanisterClientResult, EvmCanisterClient, EvmResult, IcCanisterClient,
};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, VirtualMemory};
use minter_did::error::Error;

use crate::constant::{DEFAULT_TX_GAS_LIMIT, NONCE_MEMORY_ID};
use crate::context::Context;
use crate::evm::Evm;
use crate::memory::MEMORY_MANAGER;

/// Interface for calling evm canister methods
#[async_trait(?Send)]
pub trait EvmCanister: Evm {
    /// Send a transaction from the current ic agent address
    async fn transact(
        &self,
        value: U256,
        to: H160,
        data: Vec<u8>,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<H256, Error>;

    /// Create a new contract with the specified code
    async fn create_contract(
        &self,
        value: U256,
        code: Vec<u8>,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<H256, Error>;

    fn reset(&self);

    fn as_evm(&self) -> &dyn Evm;
}

/// EVM canister implementation that calls the remote EVM canister.
#[derive(Default)]
pub struct EvmCanisterImpl {}

impl EvmCanisterImpl {
    /// Returns EVM client
    pub fn get_evm_client(&self, context: &dyn Context) -> EvmCanisterClient<IcCanisterClient> {
        let principal = context.get_state().config.get_evm_principal();
        let canister = IcCanisterClient::new(principal);
        EvmCanisterClient::new(canister)
    }

    /// Returns next nonce value
    fn get_nonce(&self) -> U256 {
        EVM_CANISTER_NONCE_CELL.with(|nonce| {
            let value = nonce.borrow().get().clone();
            nonce
                .borrow_mut()
                .set(value.clone() + U256::one())
                .expect("failed to update nonce");
            value
        })
    }

    /// Wraps IC call to EVM
    pub fn process_call<T>(&self, result: Result<T, CanisterClientError>) -> Result<T, Error> {
        result.map_err(|e| Error::Internal(format!("ic call failure: {e:?}")))
    }

    /// Wraps the result of the IC EVM code
    pub fn process_call_result<T>(
        &self,
        result: CanisterClientResult<EvmResult<T>>,
    ) -> Result<T, Error> {
        let result = self.process_call(result)?;
        if let Err(EvmError::TransactionPool(TransactionPoolError::InvalidNonce {
            expected, ..
        })) = &result
        {
            EVM_CANISTER_NONCE_CELL.with(|nonce| {
                nonce
                    .borrow_mut()
                    .set(expected.clone())
                    .expect("failed to update nonce");
            });
        }

        result.map_err(|e| Error::Internal(format!("transaction error: {e}")))
    }

    /// Creates transaction params for the given transaction value
    async fn get_transaction(
        &self,
        to: Option<H160>,
        value: U256,
        data: Vec<u8>,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<Transaction, Error> {
        // NOTE: this is a workaround for clippy "borrow reference held across await point"
        // For some reason clippy produces a false warning for the code
        // let context = context.borrow();
        // ...
        // drop(context); // before the first await point
        let (signer, gas_price, chain_id) = {
            let context = context.borrow();
            let signer = context.get_state().signer.get_transaction_signer();
            let gas_price = context.get_state().config.get_evm_gas_price();
            let chain_id = context.get_state().config.get_evmc_chain_id();

            (signer, gas_price, chain_id)
        };

        let from = signer
            .get_address()
            .await
            .map_err(|e| Error::from(format!("failed to get address: {e}")))?;

        let mut transaction = ethers_core::types::Transaction {
            from: from.into(),
            to: to.map(Into::into),
            nonce: self.get_nonce().0,
            value: value.0,
            gas: DEFAULT_TX_GAS_LIMIT.into(),
            gas_price: Some(gas_price.into()),
            input: data.into(),
            chain_id: Some(chain_id.into()),
            ..Default::default()
        };
        let typed_transaction: TypedTransaction = (&transaction).into();

        let signature = signer
            .sign_transaction(&typed_transaction)
            .await
            .map_err(|e| Error::from(format!("failed to sign transaction: {e}")))?;

        transaction.r = signature.r.into();
        transaction.s = signature.s.into();
        transaction.v = signature.v.into();

        transaction.hash = transaction.hash();

        Ok(transaction.into())
    }
}

#[async_trait(?Send)]
impl Evm for EvmCanisterImpl {
    async fn send_raw_transaction(
        &self,
        tx: Transaction,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<H256, Error> {
        let evm = self.get_evm_client(&*context.borrow());

        self.process_call_result(evm.send_raw_transaction(tx).await)
    }

    async fn get_contract_code(
        &self,
        address: H160,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<Vec<u8>, Error> {
        let evm = self.get_evm_client(&*context.borrow());
        self.process_call_result(evm.eth_get_code(address, BlockNumber::Latest).await)
            .and_then(|code| {
                hex::decode(code)
                    .map_err(|_| Error::Internal("failed to decode contract code".to_string()))
            })
    }

    async fn get_balance(
        &self,
        address: H160,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<U256, Error> {
        let evm = self.get_evm_client(&*context.borrow());
        self.process_call(evm.account_basic(address).await)
            .map(|acc| acc.balance)
    }

    async fn get_transaction_by_hash(
        &self,
        tx_hash: H256,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<Option<Transaction>, Error> {
        let evm = self.get_evm_client(&*context.borrow());
        self.process_call(evm.eth_get_transaction_by_hash(tx_hash).await)
    }

    async fn get_transaction_receipt_by_hash(
        &self,
        tx_hash: H256,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<Option<TransactionReceipt>, Error> {
        let evm = self.get_evm_client(&*context.borrow());
        self.process_call_result(evm.eth_get_transaction_receipt(tx_hash).await)
    }

    async fn get_transaction_count(
        &self,
        address: H160,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<U256, Error> {
        let evm = self.get_evm_client(&*context.borrow());
        self.process_call(evm.account_basic(address).await)
            .map(|acc| acc.nonce)
    }

    /// Returns the transaction receipt by the transaction hash
    #[allow(clippy::too_many_arguments)]
    async fn eth_call(
        &self,
        from: Option<H160>,
        to: Option<H160>,
        value: Option<U256>,
        gas_limit: u64,
        gas_price: Option<U256>,
        data: Option<Bytes>,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<String, Error> {
        let evm = self.get_evm_client(&*context.borrow());
        self.process_call_result(
            evm.eth_call(from, to, value, gas_limit, gas_price, data)
                .await,
        )
    }

    async fn eth_chain_id(&self, context: &Rc<RefCell<dyn Context>>) -> Result<u64, Error> {
        let evm = self.get_evm_client(&*context.borrow());
        self.process_call(evm.eth_chain_id().await)
    }
}

#[async_trait(?Send)]
impl EvmCanister for EvmCanisterImpl {
    async fn transact(
        &self,
        value: U256,
        to: H160,
        data: Vec<u8>,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<H256, Error> {
        let tx = self.get_transaction(Some(to), value, data, context).await?;

        self.send_raw_transaction(tx, context).await
    }

    async fn create_contract(
        &self,
        value: U256,
        code: Vec<u8>,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<H256, Error> {
        let tx = self.get_transaction(None, value, code, context).await?;
        self.send_raw_transaction(tx, context).await
    }

    fn reset(&self) {
        EVM_CANISTER_NONCE_CELL.with(|nonce| {
            nonce
                .borrow_mut()
                .set(U256::one())
                .expect("failed to update nonce");
        });
    }

    fn as_evm(&self) -> &dyn Evm {
        self
    }
}

thread_local! {
    /// Current nonce value for EVM canister calls
    static EVM_CANISTER_NONCE_CELL: RefCell<StableCell<U256, VirtualMemory<DefaultMemoryImpl>>> = {
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(NONCE_MEMORY_ID)), U256::zero())
            .expect("stable memory nonce initialization failed"))
    };
}
