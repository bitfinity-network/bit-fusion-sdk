use alloy::primitives::{Address, B256};
use alloy::rpc::types::TransactionRequest;
use alloy_sol_types::{SolInterface, SolValue};
use bridge_did::evm_link::EvmLink;
use bridge_did::id256::Id256;
use bridge_utils::WrappedToken::{WrappedTokenCalls, decimalsCall, nameCall, symbolCall};
use clap::{Args, Subcommand};
use did::{BlockNumber, Bytes};
use eth_signer::LocalWallet;
use ethereum_json_rpc_client::EthJsonRpcClient;
use ethereum_json_rpc_client::reqwest::ReqwestClient;
use tracing::info;

use crate::contracts::{IcNetwork, NetworkConfig, SolidityContractDeployer};

#[derive(Debug, Subcommand)]
pub enum WrapTokenType {
    Erc20(WrapErc20Args),
}

impl WrapTokenType {
    pub async fn wrap(
        &self,
        network: IcNetwork,
        pk: B256,
        evm_link: EvmLink,
    ) -> anyhow::Result<()> {
        let base_token_parameters = self.get_base_token_parameters(pk).await?;
        let wrapped_token_address = self
            .deploy_wrapped_token(&base_token_parameters, network, pk, evm_link)
            .await?;

        info!(
            "Wrapped token contract for ERC20 token {} deployed",
            base_token_parameters.name
        );
        info!(
            "Wrapped token address: 0x{}",
            hex::encode(wrapped_token_address)
        );

        Ok(())
    }

    async fn get_base_token_parameters(&self, pk: B256) -> anyhow::Result<TokenParameters> {
        match self {
            WrapTokenType::Erc20(erc) => {
                Self::get_erc20_params(erc.base_evm_url.clone(), &erc.token_address, &pk).await
            }
        }
    }

    fn wrapped_btf_address(&self) -> &Address {
        match self {
            WrapTokenType::Erc20(WrapErc20Args {
                wrapped_btf_address,
                ..
            }) => wrapped_btf_address,
        }
    }

    async fn deploy_wrapped_token(
        &self,
        base_token_params: &TokenParameters,
        evm_network: IcNetwork,
        pk: B256,
        evm_link: EvmLink,
    ) -> anyhow::Result<Address> {
        let deployer = SolidityContractDeployer::new(
            NetworkConfig {
                bridge_network: evm_network,
                custom_network: None,
            },
            pk,
            evm_link,
        );
        let TokenParameters {
            name,
            symbol,
            decimals,
            id,
        } = base_token_params;
        deployer.deploy_wrapped_token(self.wrapped_btf_address(), name, symbol, *decimals, *id)
    }

    async fn get_erc20_params(
        evm_url: String,
        token_address: &Address,
        pk: &B256,
    ) -> anyhow::Result<TokenParameters> {
        let client = EthJsonRpcClient::new(ReqwestClient::new(evm_url));
        let wallet = LocalWallet::from_bytes(pk).expect("Cannot create ETH wallet");

        let name = String::abi_decode(
            &Self::request_contract(
                &client,
                &wallet,
                token_address,
                WrappedTokenCalls::name(nameCall {}).abi_encode().into(),
            )
            .await?,
            true,
        )?;
        let symbol = String::abi_decode(
            &Self::request_contract(
                &client,
                &wallet,
                token_address,
                WrappedTokenCalls::symbol(symbolCall {}).abi_encode().into(),
            )
            .await?,
            true,
        )?;
        let decimals = u32::abi_decode(
            &Self::request_contract(
                &client,
                &wallet,
                token_address,
                WrappedTokenCalls::decimals(decimalsCall {})
                    .abi_encode()
                    .into(),
            )
            .await?,
            true,
        )? as u8;
        let chain_id = client.get_chain_id().await?;

        let id = Id256::from_evm_address(&did::H160::new(*token_address), chain_id as u32);

        Ok(TokenParameters {
            name,
            symbol,
            decimals,
            id,
        })
    }

    async fn request_contract(
        client: &EthJsonRpcClient<ReqwestClient>,
        wallet: &LocalWallet,
        address: &Address,
        data: Bytes,
    ) -> anyhow::Result<Vec<u8>> {
        let data = data.0.to_vec();

        let result = client
            .eth_call(
                &TransactionRequest {
                    from: Some(wallet.address()),
                    to: Some((*address).into()),
                    gas: None,
                    gas_price: None,
                    value: None,
                    input: data.into(),
                    nonce: None,
                    chain_id: None,
                    ..Default::default()
                },
                BlockNumber::Finalized,
            )
            .await?;

        let hex = hex::decode(result.trim_start_matches("0x"))?;
        Ok(hex)
    }
}

struct TokenParameters {
    name: String,
    symbol: String,
    decimals: u8,
    id: Id256,
}

#[derive(Debug, Args)]
pub struct WrapErc20Args {
    #[arg(long)]
    base_evm_url: String,

    #[arg(long)]
    base_btf_address: Address,

    #[arg(long)]
    wrapped_btf_address: Address,

    #[arg(long)]
    token_address: Address,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::TESTNET_URL;

    #[tokio::test]
    #[ignore = "requires connection to BF testnet and some BTF in the wallet; if in doubt, run manually"]
    async fn getting_erc20_token_parameters() {
        let wallet_pk = "0xf886b8c9418002fb34bbeaa0a1002e13eebd9fdc0fc41d70a7393747cd95af71";
        let token_address = "0xCe72ce5Aa299e1E630CBf5262Dd630260b42BF1a";
        let evm_url = TESTNET_URL;

        let token_address = hex::decode(token_address.trim_start_matches("0x")).unwrap();
        let token_address = Address::from_slice(&token_address);
        let wallet_pk = hex::decode(wallet_pk.trim_start_matches("0x")).unwrap();
        let wallet_pk = B256::from_slice(&wallet_pk);

        let token_params =
            WrapTokenType::get_erc20_params(evm_url.into(), &token_address, &wallet_pk)
                .await
                .unwrap();

        assert_eq!(token_params.name, "Maximium");
        assert_eq!(token_params.symbol, "MXM");
        assert_eq!(token_params.decimals, 18);
        assert_eq!(token_params.id.to_evm_address().unwrap().1.0, token_address);
    }
}
