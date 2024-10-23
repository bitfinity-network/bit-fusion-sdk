/// A task for deploying the fee charge contract

import {task} from 'hardhat/config';

task(
    'deploy-wrapped-token',
    'Deploys a wrapped token contract through BFT bridge contract API',
)
    .addParam(
        'bftBridge',
        'The addresses of the BFT bridge contract',
    )
    .addParam('name', 'The name of the token')
    .addParam('symbol', 'The symbol of the token')
    .addParam('decimals', 'The decimals of the token')
    .addParam('baseTokenId', 'ID256 of the base token')
    .setAction(
        async ({bftBridge, name, symbol, decimals, baseTokenId}, hre) => {
            const {network} = hre.hardhatArguments;

            if (!network) {
                throw new Error('Please specify a network');
            }

            const [deployer] = await hre.ethers.getSigners();
            const contract = await hre.ethers.getContractAt(
                'BFTBridge',
                bftBridge,
                deployer,
            );

            // deploy erc20
            const response = await contract.deployERC20(
                name,
                symbol,
                decimals,
                baseTokenId,
            );
            const receipt = await response.wait();
            // Make sure the receipt status is 1

            if (!receipt || receipt.status !== 1) {
                throw new Error('Failed to deploy ERC20 token contract');
            }

            const tokenAddress = await receipt.getResult();
            console.log('ERC20 deployed at:', tokenAddress);
        },
    );
