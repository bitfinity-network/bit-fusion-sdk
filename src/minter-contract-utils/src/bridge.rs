use core::fmt;

use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::{Transaction, H256};
use ic_task_scheduler::task::TaskOptions;
use minter_did::{
    id256::Id256,
    order::{MintOrder, SignedMintOrder},
};

use crate::{
    bft_bridge_api::{BurntEventData, MintedEventData, NotifyMinterEventData},
    evm_link::EvmLink,
    operation_store::{MinterOperation, OperationId},
};

pub type BftResult<T> = Result<T, Error>;

pub trait BftBridge {
    type BaseBurn: fmt::Debug;
    type BaseMint: fmt::Debug;

    fn context(&self) -> impl BridgeContext<Operation<Self::BaseBurn, Self::BaseMint>>;

    async fn progress_base_burn(
        &mut self,
        id: OperationId,
        op: Self::BaseBurn,
    ) -> Result<(), Error>;

    async fn start_base_mint(&mut self, event: BurntEventData) -> Result<Self::BaseMint, Error>;

    async fn progress_base_mint(&mut self, op: Self::BaseMint) -> Result<(), Error>;

    async fn on_bft_bridge_notification(
        &mut self,
        _notfication: NotifyMinterEventData,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn on_wrapped_token_burnt(&mut self, event: BurntEventData) -> Result<(), Error> {
        let id = self
            .context()
            .create_operation(Operation::Withdraw(WithdrawOperation::WrappedBurnt(event)));
        log::trace!("Withdraw operations #{id} created due to wrapped token burn.");

        self.context()
            .schedule_operation(id, TaskOptions::default());

        Ok(())
    }

    async fn on_wrapped_token_minted(&mut self, event: MintedEventData) -> BftResult<()> {
        let operation_id = self
            .context()
            .get_operation_id_by_address(event.recipient, event.nonce)?;
        let operation = self.context().get_operation(operation_id)?;
        let Operation::Deposit(DepositOperation::MintOrderSent(order_sent)) = operation else {
            log::warn!("Expecte DepositOperation::MintOrderSent, found {operation:?}");
            return Err(Error::UnexpectedOperationState);
        };

        self.context().update_operation(
            operation_id,
            Operation::Deposit(DepositOperation::Minted(order_sent.into())),
        )
    }

    async fn progress(&mut self, id: OperationId) -> BftResult<()> {
        let operation = self.context().get_operation(id)?;

        match operation {
            Operation::Deposit(op) => self.progress_deposit(id, op).await,
            Operation::Withdraw(op) => self.progress_withdraw(id, op).await,
        }
    }

    async fn progress_deposit(
        &mut self,
        id: OperationId,
        op: DepositOperation<Self::BaseBurn>,
    ) -> BftResult<()> {
        match op {
            DepositOperation::BaseBurn(op) => self.progress_base_burn(id, op).await,
            DepositOperation::BaseBurnt(op) => self.progress_base_burnt(id, op).await,
            DepositOperation::MintOrderSigned(op) => self.progress_mint_order_signed(id, op).await,
            DepositOperation::MintOrderSent(op) => self.progress_mint_sent(id, op).await,
            DepositOperation::Minted(op) => self.progress_minted(id, op).await,
        }
    }

    async fn progress_base_burnt(&mut self, id: OperationId, op: BaseBurntState) -> BftResult<()> {
        log::trace!("Signing MintOrder: {:?}", op.order);

        let signer = self.context().get_transaction_signer();
        let signed_mint_order = op
            .order
            .encode_and_sign(&signer)
            .await
            .map_err(|e| Error::Signing(e.to_string()))?;

        let op = DepositOperation::MintOrderSigned(MintOrderSignedState {
            token_id: op.order.src_token,
            amount: op.order.amount,
            signed_mint_order: Box::new(signed_mint_order),
        });

        self.context()
            .update_operation(id, Operation::Deposit(op))?;
        self.context()
            .schedule_operation(id, TaskOptions::default());

        Ok(())
    }

    async fn progress_mint_order_signed(
        &mut self,
        id: OperationId,
        op: MintOrderSignedState,
    ) -> BftResult<()> {
        let ctx = self.context();
        let mut tx = ctx.mint_transaction_data(&op.signed_mint_order);

        let signature = ctx
            .get_transaction_signer()
            .sign_transaction(&(&tx).into())
            .await
            .map_err(|e| Error::Signing(e.to_string()))?;

        tx.r = signature.r.0;
        tx.s = signature.s.0;
        tx.v = signature.v.0;
        tx.hash = tx.hash();

        let tx_id = ctx
            .wrapped_evm()
            .get_json_rpc_client()
            .send_raw_transaction(tx)
            .await
            .map_err(|e| Error::WrappedToken(e.to_string()))?;

        let op = DepositOperation::MintOrderSent(MintOrderSentState {
            token_id: op.token_id,
            amount: op.amount,
            signed_mint_order: op.signed_mint_order,
            tx_id,
        });

        self.context()
            .update_operation(id, Operation::Deposit(op))?;

        Ok(())
    }

    async fn progress_mint_sent(
        &mut self,
        _id: OperationId,
        _op: MintOrderSentState,
    ) -> BftResult<()> {
        log::warn!("Deposit operation in MintOrderSentState should not be scheduled to progress by default.");
        Ok(())
    }

    async fn progress_minted(&mut self, _id: OperationId, _op: MintedState) -> BftResult<()> {
        log::debug!(
            "Deposit operation in MintedState should not be scheduled to progress by default."
        );
        Ok(())
    }

    async fn progress_withdraw(
        &mut self,
        id: OperationId,
        op: WithdrawOperation<Self::BaseMint>,
    ) -> BftResult<()> {
        match op {
            WithdrawOperation::WrappedBurnt(burnt) => {
                let base_mint = self.start_base_mint(burnt).await?;
                let base_mint = WithdrawOperation::BaseMint(base_mint);
                self.context()
                    .update_operation(id, Operation::Withdraw(base_mint))?;
                self.context()
                    .schedule_operation(id, TaskOptions::default());
                Ok(())
            }
            WithdrawOperation::BaseMint(state) => self.progress_base_mint(state).await,
        }
    }
}

#[derive(Debug)]
pub enum Operation<BaseBurn, BaseMint> {
    Deposit(DepositOperation<BaseBurn>),
    Withdraw(WithdrawOperation<BaseMint>),
}

impl<BaseBurn, BaseMint> MinterOperation for Operation<BaseBurn, BaseMint>
where
    BaseBurn: MinterOperation,
    BaseMint: MinterOperation,
{
    fn is_complete(&self) -> bool {
        match self {
            Operation::Deposit(op) => op.is_complete(),
            Operation::Withdraw(op) => op.is_complete(),
        }
    }
}

#[derive(Debug)]
pub enum DepositOperation<BaseBurnState> {
    BaseBurn(BaseBurnState),
    BaseBurnt(BaseBurntState),
    MintOrderSigned(MintOrderSignedState),
    MintOrderSent(MintOrderSentState),
    Minted(MintedState),
}

impl<BaseBurntState> MinterOperation for DepositOperation<BaseBurntState>
where
    BaseBurntState: MinterOperation,
{
    fn is_complete(&self) -> bool {
        match self {
            DepositOperation::BaseBurn(op) => op.is_complete(),
            DepositOperation::BaseBurnt(op) => op.is_complete(),
            DepositOperation::MintOrderSigned(op) => op.is_complete(),
            DepositOperation::MintOrderSent(op) => op.is_complete(),
            DepositOperation::Minted(op) => op.is_complete(),
        }
    }
}

#[derive(Debug)]
pub struct BaseBurntState {
    pub order: MintOrder,
}

impl MinterOperation for BaseBurntState {
    fn is_complete(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct MintOrderSignedState {
    pub token_id: Id256,
    pub amount: U256,
    pub signed_mint_order: Box<SignedMintOrder>,
}

impl MinterOperation for MintOrderSignedState {
    fn is_complete(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct MintOrderSentState {
    pub token_id: Id256,
    pub amount: U256,
    pub signed_mint_order: Box<SignedMintOrder>,
    pub tx_id: H256,
}

impl MinterOperation for MintOrderSentState {
    fn is_complete(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct MintedState {
    pub token_id: Id256,
    pub amount: U256,
    pub tx_id: H256,
}

impl From<MintOrderSentState> for MintedState {
    fn from(value: MintOrderSentState) -> Self {
        Self {
            token_id: value.token_id,
            amount: value.amount,
            tx_id: value.tx_id,
        }
    }
}

impl MinterOperation for MintedState {
    fn is_complete(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub enum WithdrawOperation<BaseMintState> {
    WrappedBurnt(BurntEventData),
    BaseMint(BaseMintState),
}

impl<BaseMintState> MinterOperation for WithdrawOperation<BaseMintState>
where
    BaseMintState: MinterOperation,
{
    fn is_complete(&self) -> bool {
        match self {
            WithdrawOperation::WrappedBurnt(_) => false,
            WithdrawOperation::BaseMint(op) => op.is_complete(),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    BaseToken { code: u32, msg: String },
    OperationNotFound(OperationId),
    UnexpectedOperationState,
    SerializationError(String),
    Signing(String),
    WrappedToken(String),
}

pub trait BridgeContext<Op> {
    fn schedule_operation(&self, id: OperationId, options: TaskOptions);

    fn get_transaction_signer(&self) -> impl TransactionSigner + 'static;

    fn mint_transaction_data(&self, order: &SignedMintOrder) -> Transaction;

    fn wrapped_evm(&self) -> EvmLink;

    fn get_operation(&self, id: OperationId) -> BftResult<Op>;
    fn get_operation_id_by_address(&self, address: H160, nonce: u32) -> BftResult<OperationId>;
    fn create_operation(&mut self, op: Op) -> OperationId;
    fn update_operation(&mut self, id: OperationId, op: Op) -> BftResult<()>;
}
