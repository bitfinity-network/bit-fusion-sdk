/// A task for deploying the fee charge contract

import { task } from 'hardhat/config';
import { int } from 'hardhat/internal/core/params/argumentTypes';

task('deploy-fee-charge', 'Deploys the fee charge contract')
    .addParam('bridges', 'The addresses of the bridges')
    .addOptionalParam('nonce', 'The nonce of the transaction', undefined, int)
    .addOptionalParam('expectedAddress', 'The expected address of the fee charge')
    .setAction(async ({ nonce, bridges, expectedAddress }, hre) => {
        const { network } = hre.hardhatArguments;

        if (!network) {
            throw new Error('Please specify a network');
        }

        let [deployer] = await hre.ethers.getSigners();

        // If nonce is not provided, get the nonce from the deployer
        if (nonce === undefined) {
            nonce = await deployer.getNonce('pending');
        }

        if (!Array.isArray(bridges) || bridges.length === 0) {
            throw new Error('Bridges must be a non-empty array of addresses');
        }

        // Validate the arguments that it is address
        for (const address of bridges) {
            if (!hre.ethers.isAddress(address)) {
                throw new Error(`Invalid address: ${address}`);
            }
        }

        const FeeCharge = await hre.ethers.getContractFactory('FeeCharge');
        const feeCharge = await FeeCharge.deploy(bridges);

        // Wait for the deployment to be confirmed
        await feeCharge.waitForDeployment();

        // Get Address
        const feeChargeAddress = await feeCharge.getAddress();

        // Check if the fee charge address is as expected
        if (expectedAddress !== undefined && feeChargeAddress !== expectedAddress) {
            console.error(
                `Expected Address: ${expectedAddress} but got ${feeChargeAddress}`
            );
        }

        console.log(`Fee charge address: ${feeChargeAddress}`);
    });
