use std::fmt::{Display, Formatter};

use BTFBridge::{BurnTokenEvent, MintTokenEvent, NotifyMinterEvent};
use alloy::primitives::Uint;
use alloy_sol_types::sol;
use candid::{CandidType, Decode};
use serde::{Deserialize, Serialize};

use crate::error::{BTFResult, Error};
use crate::op_id::OperationId;
use crate::operation_log::Memo;

sol! {
    #[derive(Debug, Serialize, Deserialize)]
    BTFBridge,
    "../../solidity/out/BTFBridge.sol/BTFBridge.json"
}

/// Emitted when token is burnt by BTFBridge.
#[derive(Debug, Default, Clone, CandidType, Serialize, Deserialize)]
pub struct BurntEventData {
    pub sender: did::H160,
    pub amount: did::U256,
    pub from_erc20: did::H160,
    pub recipient_id: Vec<u8>,
    pub to_token: Vec<u8>,
    pub operation_id: u32,
    pub name: Vec<u8>,
    pub symbol: Vec<u8>,
    pub decimals: u8,
    pub memo: Vec<u8>,
}

impl BurntEventData {
    pub fn memo(&self) -> Option<Memo> {
        if self.memo.is_empty() {
            None
        } else if self.memo.len() == 32 {
            Some(
                self.memo
                    .as_slice()
                    .try_into()
                    .expect("should be exactly 32 bytes"),
            )
        } else {
            None
        }
    }
}

impl From<BurnTokenEvent> for BurntEventData {
    fn from(event: BurnTokenEvent) -> Self {
        Self {
            sender: event.sender.0.0.into(),
            amount: did::U256(Uint::from_limbs_slice(event.amount.as_limbs())),
            from_erc20: event.fromERC20.0.0.into(),
            recipient_id: event.recipientID.into(),
            to_token: event.toToken.0.into(),
            operation_id: event.operationID,
            name: event.name.0.into(),
            symbol: event.symbol.0.into(),
            decimals: event.decimals,
            memo: event.memo.0.into(),
        }
    }
}

/// Event emitted when token is minted by BTFBridge.
#[derive(Debug, Default, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct MintedEventData {
    pub amount: did::U256,
    pub from_token: Vec<u8>,
    pub sender_id: Vec<u8>,
    pub to_erc20: did::H160,
    pub recipient: did::H160,
    pub nonce: u32,
    pub fee_charged: did::U256,
}

impl From<MintTokenEvent> for MintedEventData {
    fn from(event: MintTokenEvent) -> Self {
        Self {
            amount: did::U256(Uint::from_limbs_slice(event.amount.as_limbs())),
            from_token: event.fromToken.0.into(),
            sender_id: event.senderID.0.into(),
            to_erc20: event.toERC20.0.0.into(),
            recipient: event.recipient.0.0.into(),
            nonce: event.nonce,
            fee_charged: did::U256(Uint::from_limbs_slice(event.chargedFee.as_limbs())),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, CandidType, Serialize, Deserialize)]
#[repr(u32)]
pub enum MinterNotificationType {
    DepositRequest = 1,
    RescheduleOperation = 2,
    Other,
}

impl From<u32> for MinterNotificationType {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::DepositRequest,
            2 => Self::RescheduleOperation,
            _ => Self::Other,
        }
    }
}

impl Display for MinterNotificationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MinterNotificationType::DepositRequest => write!(f, "DepositRequest"),
            MinterNotificationType::RescheduleOperation => write!(f, "RescheduleOperation"),
            MinterNotificationType::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, CandidType, Serialize, Deserialize)]
pub struct NotifyMinterEventData {
    pub notification_type: MinterNotificationType,
    pub tx_sender: did::H160,
    pub user_data: Vec<u8>,
    pub memo: Vec<u8>,
}

impl NotifyMinterEventData {
    /// Returns the memo of this [`NotifyMinterEventData`].
    ///
    /// # Panics
    ///
    /// Panics if the memo is not exactly 32 bytes.
    pub fn memo(&self) -> Option<Memo> {
        if self.memo.is_empty() {
            None
        } else if self.memo.len() == 32 {
            Some(
                self.memo
                    .as_slice()
                    .try_into()
                    .expect("should be exactly 32 bytes"),
            )
        } else {
            None
        }
    }

    /// Tries to decode the notification into rescheduling operation id.
    pub fn try_decode_reschedule_operation_id(&self) -> BTFResult<OperationId> {
        if self.notification_type != MinterNotificationType::RescheduleOperation {
            return Err(Error::Serialization(format!(
                "expected MinterNotificationType::RescheduleOperation, got {:?}",
                self.notification_type,
            )));
        }

        let decoded = Decode!(&self.user_data, OperationId).map_err(|e| {
            Error::Serialization(format!("failed to decode reschedule operation ID: {e}"))
        })?;
        Ok(decoded)
    }
}

impl From<NotifyMinterEvent> for NotifyMinterEventData {
    fn from(event: NotifyMinterEvent) -> Self {
        Self {
            notification_type: event.notificationType.into(),
            tx_sender: event.txSender.0.0.into(),
            user_data: event.userData.0.into(),
            memo: event.memo.0.into(),
        }
    }
}
