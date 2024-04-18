use candid::CandidType;
use did::{H160, H256};
use minter_did::order::SignedMintOrder;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::inscriber_api::{InscribeTransactions, Multisig, Protocol};

#[derive(Error, CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum BridgeError {
    #[error("{0}")]
    InscriptionParsing(String),
    #[error("{0}")]
    MalformedAddress(String),
    #[error("{0}")]
    GetDepositAddress(String),
    #[error("{0}")]
    FetchBrc20TokenDetails(String),
    #[error("{0}")]
    GetTransactionById(String),
    #[error("{0}")]
    PublicKeyFromStr(String),
    #[error("{0}")]
    AddressFromPublicKey(String),
    #[error("{0}")]
    EcdsaPublicKey(String),
    #[error("{0}")]
    SetTokenSymbol(String),
    #[error("{0}")]
    Brc20Burn(String),
    #[error("{0}")]
    Erc20Mint(#[from] Erc20MintError),
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct Brc20TokenDetails {
    pub ticker: String,
    pub holder: String,
    pub tx_id: String,
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct InscribeBrc20Args {
    pub inscription_type: Protocol,
    pub inscription: String,
    pub leftovers_address: String,
    pub dst_address: String,
    pub multisig_config: Option<Multisig>,
}

/// Arguments to `Brc20Task::MintErc20`
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MintErc20Args {
    /// User's ETH address
    pub eth_address: H160,
    /// User's BTC address
    pub btc_address: String,
    /// BRC20 token name (ticker)
    pub brc20_token: String,
}

/// Status of an ERC20 to a BRC20 swap
#[derive(CandidType, Clone, Debug, Deserialize)]
pub struct Brc20InscribeStatus {
    /// commit_txid and reveal_txid
    pub tx_ids: InscribeTransactions,
}

/// Errors that occur during an ERC20 to a BRC20 swap.
#[derive(Error, CandidType, Clone, Debug, Deserialize)]
pub enum Brc20InscribeError {
    /// Error from the Inscriber regarding a BRC20 transfer call
    #[error("{0}")]
    Brc20Transfer(String),
    /// Error returned by the `inscribe` endpoint of the Inscriber.
    #[error("{0}")]
    Inscribe(String),
    /// There are too many concurrent requests, retry later.
    #[error("{0}")]
    TemporarilyUnavailable(String),
}

/// Status of a BRC20 to ERC20 swap
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq)]
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

/// Errors that occur during a BRC20 to ERC20 swap.
#[derive(Error, Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub enum Erc20MintError {
    /// Error from the Brc20Bridge
    #[error("{0}")]
    Brc20Bridge(String),
    /// The Brc20Bridge is not properly initialized.
    #[error("{0}")]
    NotInitialized(String),
    /// Error connecting to the EVM.
    #[error("{0}")]
    Evm(String),
    /// The inscription (BRC20) received is invalid.
    #[error("{0}")]
    InvalidBrc20(String),
    /// The specified amount for the ERC20 is smaller than the fee.
    /// The transaction will not be precessed.
    #[error("{0}")]
    ValueTooSmall(String),
    /// Error while signing the mint order.
    #[error("{0}")]
    Sign(String),
}
