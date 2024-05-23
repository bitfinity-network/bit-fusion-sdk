use candid::CandidType;
use did::{H160, H256};
use inscriber::interface::InscribeTransactions;
use minter_did::order::SignedMintOrder;
use serde::{Deserialize, Serialize};

/// Arguments to `Brc20Task::MintErc20`
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MintErc20Args {
    /// User's ETH address
    pub eth_address: H160,
    /// BRC20 token info
    pub brc20_token: DepositBrc20Args,
}

/// Status of a BRC20 to ERC20 swap
#[derive(Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum Erc20MintStatus {
    /// This happens when the transaction is processed, the BRC20 inscription is parsed and validated,
    /// and the mint order is created; however, there is a problem sending the mint order to the EVM.
    /// The signed mint order can be sent manually to the BftBridge to mint wrapped tokens.
    Signed(Box<SignedMintOrder>),
    /// Mint order for wrapped tokens is successfully sent to the `BftBridge`.
    Minted {
        /// Amount of tokens minted.
        amount: u64,
        /// EVM transaction ID.
        tx_id: H256,
    },
}

#[derive(Debug, CandidType, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub struct DepositBrc20Args {
    pub tx_id: String,
    pub ticker: String,
}

/// Status of an ERC20 to a BRC20 swap
#[derive(CandidType, Clone, Debug, Deserialize)]
pub struct Brc20InscribeStatus {
    pub tx_ids: InscribeTransactions,
}

#[derive(Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum DepositError {
    InscriptionParser(String),
    FetchBrc20Token(String),
    BadHttpRequest,
    GetTransactionById(String),
    SetTokenSymbol(String),
    NothingToDeposit,
    ValueTooSmall(String),
    Unavailable(String),
    Pending {
        min_confirmations: u32,
        current_confirmations: u32,
    },
    NotInitialized(String),
    MintOrderSign(String),
    FetchUtxos(String),
    Evm(String),
}

#[derive(Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum WithdrawError {
    NoSuchInscription(String),
    InscriptionTransfer(String),
    InvalidInscription(String),
    MalformedAddress(String),
}
