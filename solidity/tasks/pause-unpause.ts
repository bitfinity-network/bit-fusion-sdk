/// A task for pausing and unpausing the contract

import { task } from "hardhat/config";

/// A task for pausing the contract
task("pause", "Pauses the contract")
    .addParam("contract", "The address of the contract to pause")
    .setAction(async (args, hre) => {
        // Make sure the network is specified
        const { network } = hre.hardhatArguments;

        if (!network) {
            throw new Error('Please specify a network');
        }

        const [deployer] = await hre.ethers.getSigners();
        const contract = await hre.ethers.getContractAt(
            'BFTBridge',
            args.contract,
            deployer
        );

        const response = await contract.pause();

        // Wait for the transaction to be confirmed
        const receipt = await response.wait();

        // Make sure the receipt status is 1
        if (receipt?.status !== 1) {
            throw new Error('Failed to pause contract');
        }

        console.log('Contract paused');
    });


/// A task for unpausing the contract
task("unpause", "Unpauses the contract")
    .addParam("contract", "The address of the contract to unpause")
    .setAction(async (args, hre) => {
        // Make sure the network is specified
        const { network } = hre.hardhatArguments;

        if (!network) {
            throw new Error('Please specify a network');
        }

        const [deployer] = await hre.ethers.getSigners();
        const contract = await hre.ethers.getContractAt(
            'BFTBridge',
            args.contract,
            deployer
        );

        const response = await contract.unpause();

        // Wait for the transaction to be confirmed
        const receipt = await response.wait();

        // Make sure the receipt status is 1
        if (receipt?.status !== 1) {
            throw new Error('Failed to unpause contract');
        }

        console.log('Contract unpaused');
    });