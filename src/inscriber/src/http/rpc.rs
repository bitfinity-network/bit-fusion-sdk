use std::future::Future;

use ethers_core::abi::ethereum_types::H520;
use ethers_core::types::{Signature, H160};
use jsonrpc_core::{Failure, MethodCall, Output, Success};
use serde_json::{json, Value};

use super::accessor::ParamsAccessors;
use crate::constant::{
    HTTP_METHOD_BRC20_TRANSFER_METHOD_NAME, HTTP_METHOD_GET_BTC_ADDRESS_METHOD_NAME,
    HTTP_METHOD_GET_INSCRIBER_FEE_METHOD_NAME, HTTP_METHOD_INSCRIBER_METHOD_NAME,
};
use crate::interface::inscriber_api::{InscribeError, InscribeResult, Multisig, Protocol};
use crate::{ops, Inscriber};

pub type RpcResult = Result<Value, InscribeError>;

/// RPC handler for the inscriber.
pub struct Rpc;

impl Rpc {
    /// Processes JSON-RPC requests by dispatching them to handler functions.
    ///
    /// Handles dispatching requests to the appropriate handler function based
    /// on the JSON-RPC method name. Also handles constructing the JSON-RPC
    /// response.
    pub async fn process_request<Fut, Function>(
        method_call: MethodCall,
        handler: &Function,
    ) -> Output
    where
        Fut: Future<Output = RpcResult>,
        Function: Fn(MethodCall) -> Fut,
    {
        let jsonrpc = method_call.jsonrpc;
        let id: jsonrpc_core::Id = method_call.id.clone();

        match handler(method_call).await {
            Ok(result) => Output::Success(Success {
                jsonrpc,
                result,
                id,
            }),
            Err(error) => Output::Failure(Failure {
                jsonrpc,
                error: jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::InternalError,
                    message: error.to_string(),
                    data: None,
                },
                id,
            }),
        }
    }

    /// Handles JSON-RPC method dispatch by matching the method name to handler functions
    pub async fn handle_calls(method_call: MethodCall) -> RpcResult {
        match method_call.method.as_str() {
            HTTP_METHOD_BRC20_TRANSFER_METHOD_NAME => Self::brc20_transfer(method_call).await,
            HTTP_METHOD_INSCRIBER_METHOD_NAME => Self::inscribe(method_call).await,
            HTTP_METHOD_GET_BTC_ADDRESS_METHOD_NAME => Self::get_bitcoin_address(method_call).await,
            HTTP_METHOD_GET_INSCRIBER_FEE_METHOD_NAME => {
                Self::get_inscription_fees(method_call).await
            }
            _ => Err(jsonrpc_core::Error::method_not_found().into()),
        }
    }

    pub async fn get_bitcoin_address(method_call: MethodCall) -> RpcResult {
        method_call.validate_params(&["expected_address", "signature", "signed_message"], 3)?;

        let expected_address: H160 = method_call.get_from_vec(0)?;
        let signature: H520 = method_call.get_from_vec(1)?;
        let signed_message: String = method_call.get_from_vec(2)?;

        let actual_address = Self::recover_pubkey(signed_message, signature)?;

        Self::verify_address(actual_address, expected_address)?;

        let derivation_path = Inscriber::derivation_path(Some(actual_address));

        let address = ops::get_bitcoin_address(derivation_path).await;

        Ok(json!(address))
    }

    pub async fn inscribe(method_call: MethodCall) -> RpcResult {
        method_call.validate_params(
            &[
                "inscription_type",
                "inscription",
                "leftovers_address",
                "dst_address",
                "expected_address",
                "signature",
                "signed_message",
            ],
            6,
        )?;

        let inscription_type: Protocol = method_call.get_from_vec(0)?;
        let inscription: String = method_call.get_from_vec(1)?;
        let leftovers_address: String = method_call.get_from_vec(2)?;
        let dst_address: String = method_call.get_from_vec(3)?;
        let expected_address: H160 = method_call.get_from_vec(4)?;
        let signature: H520 = method_call.get_from_vec(5)?;
        let signed_message: String = method_call.get_from_vec(6)?;
        let multisig_config: Option<Multisig> = method_call.get_from_vec(7).ok();

        let actual_address = Self::recover_pubkey(signed_message, signature)?;

        Self::verify_address(actual_address, expected_address)?;

        let derivation_path = Inscriber::derivation_path(Some(actual_address));

        let inscription = ops::inscribe(
            inscription_type,
            inscription,
            leftovers_address,
            dst_address,
            multisig_config,
            derivation_path,
        )
        .await?;

        Ok(json!(inscription))
    }

    pub async fn brc20_transfer(method_call: MethodCall) -> RpcResult {
        method_call.validate_params(
            &[
                "inscription",
                "leftovers_address",
                "recipient_address",
                "expected_address",
                "signature",
                "signed_message",
            ],
            6,
        )?;

        let inscription: String = method_call.get_from_vec(0)?;
        let leftovers_address: String = method_call.get_from_vec(1)?;
        let dst_address: String = method_call.get_from_vec(2)?;
        let expected_address: H160 = method_call.get_from_vec(3)?;
        let signature: H520 = method_call.get_from_vec(4)?;
        let signed_message: String = method_call.get_from_vec(5)?;
        let multisig_config: Option<Multisig> = method_call.get_from_vec(6).ok();

        let actual_address = Self::recover_pubkey(signed_message, signature)?;

        Self::verify_address(actual_address, expected_address)?;

        let derivation_path = Inscriber::derivation_path(Some(actual_address));

        let result = ops::brc20_transfer(
            inscription,
            leftovers_address,
            dst_address,
            multisig_config,
            derivation_path,
        )
        .await?;

        Ok(json!(result))
    }

    pub async fn get_inscription_fees(method_call: MethodCall) -> RpcResult {
        method_call.validate_params(&["inscription_type", "inscription"], 2)?;

        let inscription_type: Protocol = method_call.get_from_vec(0)?;
        let inscription: String = method_call.get_from_vec(1)?;
        let multisig_config: Option<Multisig> = method_call.get_from_vec(2).ok();

        let fees =
            ops::get_inscription_fees(inscription_type, inscription, multisig_config).await?;

        Ok(json!(fees))
    }

    /// Recovers the public key from a signed message and signature.
    pub fn recover_pubkey(message: String, signature: H520) -> InscribeResult<H160> {
        let signature = Signature::try_from(signature.as_bytes())?;
        let address = signature.recover(message)?;

        Ok(address)
    }

    /// Verifies that the actual address matches the expected address.
    pub fn verify_address(actual_address: H160, expected_address: H160) -> InscribeResult<()> {
        if actual_address != expected_address {
            return Err(InscribeError::AddressMismatch {
                expected: expected_address.to_string(),
                actual: actual_address.to_string(),
            });
        }

        Ok(())
    }
}
