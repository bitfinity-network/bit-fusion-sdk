import { task } from 'hardhat/config';
import { HardhatRuntimeEnvironment } from 'hardhat/types';
import { Contract, ContractFactory, keccak256 } from 'ethers';
import { DeployImplementationResponse } from '@openzeppelin/hardhat-upgrades/dist/deploy-implementation';

interface UpgradeBftParams {
    proxyAddress: string;
    referenceContract: string;
    updatedContract: string;
}

/**
   * Upgrades the BFT contract to a new implementation.
   *
   * @param proxyAddress - The address of the BFT proxy contract to be upgraded.
    * @param referenceContract - The reference contract name to be used.
    * @param isWrappedSide - The side of the bridge to be upgraded.
   * @returns The upgraded BFT contract.
   */
task('upgrade-bft-manual', 'Upgrades the BFT contract')
    .addParam('proxyAddress', 'The address of the BFT proxy contract')
    .addParam('referenceContract', 'The reference contract name to be used')
    .addParam('updatedContract', 'The updated contract name')
    .setAction(async (
        { proxyAddress, referenceContract, updatedContract }: UpgradeBftParams,
        hre: HardhatRuntimeEnvironment
    ): Promise<void> => {
        try {
            console.log('Starting BFT contract upgrade process...');

            console.log('Compiling contract...');
            await hre.run('compile');
            console.log('Contract compiled successfully.');

            const { network } = hre.hardhatArguments;

            if (!network) {
                throw new Error('Network not specified. Please provide a network.');
            }

            console.log('Deploying new implementation contract...');

            //! Change this to the new implementation contract
            const updatedBridgeContract = await hre.ethers.getContractFactory(updatedContract);


            /// Make sure you use the proxy contract address to get the
            /// contract instance
            /// and the old implementation contract should be the one that is currently deployed
            const referenceBridgeContract = await hre.ethers.getContractAt(referenceContract, proxyAddress);

            const newImplementationDeployment: DeployImplementationResponse =
                await hre.upgrades.prepareUpgrade(proxyAddress, updatedBridgeContract, {
                    kind: 'uups'
                });

            const newImplementationAddress: string = typeof newImplementationDeployment === 'string' ? newImplementationDeployment : await newImplementationDeployment.wait().then((tx) => tx?.contractAddress!);

            console.log(`New implementation contract deployed at: ${newImplementationAddress}`);

            console.log('Adding new implementation to the proxy contract...');
            let deployedByteCode = await hre.ethers.provider.getCode(
                newImplementationAddress);

            /// Get the bytecode hash
            let bytecodeHash = keccak256(deployedByteCode);

            let res = await referenceBridgeContract.addAllowedImplementation(bytecodeHash);

            let receipt = await res.wait();

            if (receipt!.status === 0) {
                throw new Error('Failed to add new implementation to the proxy contract.');
            }

            console.log('New implementation added successfully.');

            console.log('Retrieving current proxy contract...');

            const proxyContract = await hre.ethers.getContractAt(
                referenceContract,
                proxyAddress
            );

            let newImpl = await hre.ethers.getContractAt(
                "BFTBridgeV2",
                newImplementationAddress);

            const initData: string = newImpl.interface.encodeFunctionData('__BridgeV2_init');

            console.log('Upgrading proxy contract to new implementation...');
            await proxyContract.upgradeToAndCall(newImplementationAddress, initData);
            console.log('Proxy contract upgraded successfully.');

            console.log('BFT contract upgrade completed.');
            console.log(`Proxy address: ${proxyAddress}`);
            console.log(`New implementation address: ${newImplementationAddress}`);
        } catch (error) {
            throw error;
        }
    });



/**
* Prepares the BFT contract for upgrade.
* This task only deploys the new implementation contract only
*
* @param proxyAddress - The address of the BFT proxy contract to be upgraded.
* @param updatedContract - The updated contract name.
* @returns The upgraded BFT contract.
*/
task('prepareUpgrade', 'Upgrades the BFT contract')
    .addParam('proxyAddress', 'The address of the BFT proxy contract')
    .addParam('updatedContract', 'The updated contract name')
    .setAction(async (
        { proxyAddress, updatedContract }: UpgradeBftParams,
        hre: HardhatRuntimeEnvironment
    ): Promise<void> => {
        try {
            console.log('Starting BFT contract upgrade process...');

            console.log('Compiling contract...');
            await hre.run('compile');
            console.log('Contract compiled successfully.');

            const { network } = hre.hardhatArguments;

            if (!network) {
                throw new Error('Network not specified. Please provide a network.');
            }

            console.log('Deploying new implementation contract...');

            //! WARN!! Change this to the new implementation contract
            const updatedBridgeContract = await hre.ethers.getContractFactory(updatedContract);

            const newImplementationDeployment: DeployImplementationResponse =
                await hre.upgrades.prepareUpgrade(proxyAddress, updatedBridgeContract, {
                    kind: 'uups'
                });

            const newImplementationAddress: string = typeof newImplementationDeployment === 'string' ? newImplementationDeployment : await newImplementationDeployment.wait().then((tx) => tx?.contractAddress!);

            console.log(`New implementation contract deployed at: ${newImplementationAddress}`);

            console.log('Adding new implementation to the proxy contract...');
            let deployedByteCode = await hre.ethers.provider.getCode(
                newImplementationAddress);

            /// Get the bytecode hash
            let bytecodeHash = keccak256(deployedByteCode);

            console.log("Bytecode hash of the new implementation contract: ", bytecodeHash);
            console.log('New implementation added successfully.');
            console.log(`New implementation address: ${newImplementationAddress}`);
        } catch (error) {
            throw error;
        }
    });

/**
 * Upgrades the BFT contract to a new implementation.
 * This task only upgrades the proxy contract to the new implementation
 * without deploying the new implementation contract.
 *
 * @param proxyAddress - The address of the BFT proxy contract to be upgraded.
 * @param updatedContractAddress - The address of the updated contract.
 * @param updatedContractName - The updated contract name.
 * @param referenceContract - The reference contract name to be used.
 * @returns The upgraded BFT contract.
 */
task('upgradeProxy', 'Upgrades the BFT contract')
    .addParam('proxyAddress', 'The address of the BFT proxy contract')
    .addParam('updatedContractAddress', 'The address of the updated contract')
    .addParam('updatedContractName', 'The updated contract name')
    .addParam('referenceContract', 'The reference contract name to be used')
    .setAction(async (
        { proxyAddress, updatedContractAddress, referenceContract, updatedContractName },
        hre: HardhatRuntimeEnvironment
    ): Promise<void> => {
        try {
            console.log('Starting BFT contract upgrade process...');

            console.log('Compiling contract...');
            await hre.run('compile');
            console.log('Contract compiled successfully.');

            const { network } = hre.hardhatArguments;

            if (!network) {
                throw new Error('Network not specified. Please provide a network.');
            }

            console.log('Retrieving current proxy contract...');

            const proxyContract = await hre.ethers.getContractAt(
                referenceContract,
                proxyAddress
            );

            let newImpl = await hre.ethers.getContractAt(
                updatedContractName,
                updatedContractAddress);

            /// Init Method should be filled with the correct init method and parameters
            const initMethod = "";
            const initData: string = newImpl.interface.encodeFunctionData(initMethod);

            console.log('Upgrading proxy contract to new implementation...');
            await proxyContract.upgradeToAndCall(updatedContractAddress, initData);
        } catch (error) {
            throw error;
        }
    });