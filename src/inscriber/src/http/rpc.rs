use std::future::Future;

use ethers_core::abi::ethereum_types::H520;
use ethers_core::types::H160;
use jsonrpc_core::{Failure, MethodCall, Output, Success};
use serde_json::{json, Value};

use crate::accessor::ParamsAccessors;
use crate::constant::{GET_BTC_ADDRESS_METHOD_NAME, INSCRIBER_METHOD_NAME};
use crate::wallet::inscription::{Multisig, Protocol};
use crate::{ops, Inscriber};

pub type RpcResult = Result<Value, jsonrpc_core::Error>;

pub struct Rpc;

impl Rpc {
    /// Processes JSON-RPC requests by dispatching them to handler functions.
    ///
    /// Handles dispatching requests to the appropriate handler function based
    /// on the JSON-RPC method name. Also handles constructing the JSON-RPC
    /// response.
    pub async fn process_request<Fut, Function>(method_call: MethodCall, f: &Function) -> Output
    where
        Fut: Future<Output = RpcResult>,
        Function: Fn(MethodCall) -> Fut,
    {
        let jsonrpc = method_call.jsonrpc;
        let id: jsonrpc_core::Id = method_call.id.clone();
        let response = f(method_call).await;

        match response {
            Ok(result) => Output::Success(Success {
                jsonrpc,
                result,
                id,
            }),
            Err(error) => Output::Failure(Failure { jsonrpc, error, id }),
        }
    }

    /// Handles JSON-RPC method dispatch by matching the method name to handler functions
    pub async fn handle_calls(method_call: MethodCall) -> RpcResult {
        match method_call.method.as_str() {
            INSCRIBER_METHOD_NAME => Self::inscribe(method_call).await,
            GET_BTC_ADDRESS_METHOD_NAME => Self::get_bitcoin_address(method_call).await,
            _ => Err(jsonrpc_core::Error::method_not_found()),
        }
    }

    pub async fn get_bitcoin_address(method_call: MethodCall) -> RpcResult {
        method_call.validate_params(&["expected_address", "signature", "signed_message"], 3)?;

        let expected_address: H160 = method_call.get_from_vec(0)?;
        let signature: H520 = method_call.get_from_vec(1)?;
        let signed_message: String = method_call.get_from_vec(2)?;

        let actual_address = Inscriber::recover_pubkey(signed_message, signature)
            .map_err(|e| jsonrpc_core::Error::invalid_params(format!("{}", e)))?;

        if actual_address != expected_address {
            return Err(jsonrpc_core::Error::invalid_params(format!(
                "address mismatch: expected: {:?}, actual: {:?}",
                expected_address, actual_address
            )));
        }

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
                "expected_address",
                "signature",
                "signed_message",
            ],
            6,
        )?;

        let inscription_type: Protocol = method_call.get_from_vec(0)?;
        let inscription: String = method_call.get_from_vec(1)?;
        let leftovers_address: String = method_call.get_from_vec(2)?;
        let expected_address: H160 = method_call.get_from_vec(3)?;
        let signature: H520 = method_call.get_from_vec(4)?;
        let signed_message: String = method_call.get_from_vec(5)?;
        let multisig_config: Option<Multisig> = method_call.get_from_vec(6).ok();
        let dst_address: Option<String> = method_call.get_from_vec(7).ok();

        let actual_address = Inscriber::recover_pubkey(signed_message, signature).map_err(|e| {
            jsonrpc_core::Error::invalid_params(format!("invalid signature: {}", e))
        })?;

        if actual_address != expected_address {
            return Err(jsonrpc_core::Error::invalid_params(format!(
                "address mismatch: expected: {:?}, actual: {:?}",
                expected_address, actual_address
            )));
        }

        let derivation_path = Inscriber::derivation_path(Some(actual_address));

        let inscription = ops::inscribe(
            inscription_type,
            inscription,
            leftovers_address,
            dst_address,
            multisig_config,
            derivation_path,
        )
        .await
        .map_err(|e| jsonrpc_core::Error::invalid_params(format!("invalid inscription: {}", e)))?;

        Ok(json!(inscription))
    }
}
