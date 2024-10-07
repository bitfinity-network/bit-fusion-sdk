import { DeployImplementationResponse } from '@openzeppelin/hardhat-upgrades/dist/deploy-implementation';
import { Contract, ContractFactory, keccak256 } from 'ethers';
import { task } from 'hardhat/config';
import { HardhatRuntimeEnvironment } from 'hardhat/types';

interface UpgradeBftParams {
  proxyAddress: string;
  referenceContract: string;
  updatedContract: string;
}

/**
 * Logs a message with a timestamp.
 * @param message - The message to log.
 */
function log(message: string): void {
  console.log(`[${new Date().toISOString()}] ${message}`);
}

/**
 * Compiles the contracts.
 * @param hre - Hardhat Runtime Environment.
 */
async function compileContracts(hre: HardhatRuntimeEnvironment): Promise<void> {
  log('Compiling contracts...');
  await hre.run('compile');
  log('Contracts compiled successfully.');
}

/**
 * Deploys a new implementation contract.
 * @param hre - Hardhat Runtime Environment.
 * @param updatedContract - The name of the updated contract.
 * @param proxyAddress - The address of the proxy contract.
 * @returns The address of the new implementation contract.
 */
async function deployNewImplementation(
  hre: HardhatRuntimeEnvironment,
  updatedContract: string,
  proxyAddress: string,
): Promise<string> {
  log('Deploying new implementation contract...');
  const updatedBridgeContract =
    await hre.ethers.getContractFactory(updatedContract);
  const newImplementationDeployment: DeployImplementationResponse =
    await hre.upgrades.prepareUpgrade(proxyAddress, updatedBridgeContract, {
      kind: 'uups',
    });
  const newImplementationAddress: string =
    typeof newImplementationDeployment === 'string'
      ? newImplementationDeployment
      : await newImplementationDeployment
          .wait()
          .then((tx) => tx?.contractAddress!);
  log(`New implementation contract deployed at: ${newImplementationAddress}`);
  return newImplementationAddress;
}

/**
 * Adds the new implementation to the proxy contract.
 * @param hre - Hardhat Runtime Environment.
 * @param referenceContract - The name of the reference contract.
 * @param proxyAddress - The address of the proxy contract.
 * @param newImplementationAddress - The address of the new implementation contract.
 */
async function addNewImplementation(
  hre: HardhatRuntimeEnvironment,
  referenceContract: string,
  proxyAddress: string,
  newImplementationAddress: string,
): Promise<void> {
  log('Adding new implementation to the proxy contract...');
  const deployedByteCode = await hre.ethers.provider.getCode(
    newImplementationAddress,
  );
  const bytecodeHash = keccak256(deployedByteCode);
  const referenceBridgeContract = await hre.ethers.getContractAt(
    referenceContract,
    proxyAddress,
  );
  const res =
    await referenceBridgeContract.addAllowedImplementation(bytecodeHash);
  const receipt = await res.wait();
  if (receipt!.status === 0) {
    throw new Error('Failed to add new implementation to the proxy contract.');
  }
  log('New implementation added successfully.');
}

/**
 * Upgrades the proxy contract to the new implementation.
 * @param hre - Hardhat Runtime Environment.
 * @param referenceContract - The name of the reference contract.
 * @param proxyAddress - The address of the proxy contract.
 * @param newImplementationAddress - The address of the new implementation contract.
 */
async function upgradeProxy(
  hre: HardhatRuntimeEnvironment,
  referenceContract: string,
  proxyAddress: string,
  newImplementationAddress: string,
): Promise<void> {
  log('Upgrading proxy contract to new implementation...');
  const proxyContract = await hre.ethers.getContractAt(
    referenceContract,
    proxyAddress,
  );
  const newImpl = await hre.ethers.getContractAt(
    'BFTBridgeV2',
    newImplementationAddress,
  );
  const initData: string =
    newImpl.interface.encodeFunctionData('__BridgeV2_init');
  await proxyContract.upgradeToAndCall(newImplementationAddress, initData);
  log('Proxy contract upgraded successfully.');
}

/**
 * Full upgrade process
 * This task combines all three steps of the upgrade process into a single operation.
 * Use this for a complete upgrade in one go, or use the individual tasks for a more controlled upgrade process.
 */
task('upgrade-bft-full', 'Upgrades the BFT contract')
  .addParam('proxyAddress', 'The address of the BFT proxy contract')
  .addParam('referenceContract', 'The reference contract name to be used')
  .addParam('updatedContract', 'The updated contract name')
  .setAction(
    async (
      { proxyAddress, referenceContract, updatedContract }: UpgradeBftParams,
      hre: HardhatRuntimeEnvironment,
    ): Promise<void> => {
      try {
        log('Starting BFT contract upgrade process...');

        await compileContracts(hre);

        const newImplementationAddress = await deployNewImplementation(
          hre,
          updatedContract,
          proxyAddress,
        );
        await addNewImplementation(
          hre,
          referenceContract,
          proxyAddress,
          newImplementationAddress,
        );
        await upgradeProxy(
          hre,
          referenceContract,
          proxyAddress,
          newImplementationAddress,
        );

        log('BFT contract upgrade completed.');
        log(`Proxy address: ${proxyAddress}`);
        log(`New implementation address: ${newImplementationAddress}`);
      } catch (error) {
        log(`Error during upgrade process: ${error}`);
        throw error;
      }
    },
  );

/**
 * Step 1: Prepare the upgrade
 * This task deploys the new implementation contract without affecting the existing proxy.
 * It's the first step in the upgrade process and can be done in advance.
 */
task('prepareUpgrade', 'Prepares the BFT contract for upgrade')
  .addParam('proxyAddress', 'The address of the BFT proxy contract')
  .addParam('updatedContract', 'The updated contract name')
  .setAction(
    async (
      { proxyAddress, updatedContract }: UpgradeBftParams,
      hre: HardhatRuntimeEnvironment,
    ): Promise<void> => {
      try {
        log('Starting BFT contract upgrade preparation...');

        await compileContracts(hre);

        const newImplementationAddress = await deployNewImplementation(
          hre,
          updatedContract,
          proxyAddress,
        );

        log(`New implementation address: ${newImplementationAddress}`);
        log('Upgrade preparation completed.');
      } catch (error) {
        log(`Error during upgrade preparation: ${error}`);
        throw error;
      }
    },
  );

/**
 * Step 2: Add new implementation
 * This task adds the bytecode hash of the new implementation to the allowed implementations list.
 * It's the second step in the upgrade process and should be done after the new implementation is deployed.
 */
task(
  'addNewImplementation',
  'Adds the new implementation to the proxy contract',
)
  .addParam('proxyAddress', 'The address of the BFT proxy contract')
  .addParam('referenceContract', 'The reference contract name to be used')
  .addParam('implAddress', 'The address of the new implementation contract')
  .setAction(
    async (
      { proxyAddress, referenceContract, implAddress },
      hre: HardhatRuntimeEnvironment,
    ): Promise<void> => {
      try {
        log('Starting process to add new implementation...');

        await compileContracts(hre);

        await addNewImplementation(
          hre,
          referenceContract,
          proxyAddress,
          implAddress,
        );

        log('New implementation added successfully.');
      } catch (error) {
        log(`Error while adding new implementation: ${error}`);
        throw error;
      }
    },
  );

/**
 * Step 3: Upgrade the proxy
 * This task upgrades the proxy to point to the new implementation.
 * It's the final step in the upgrade process and should only be done after the new implementation is added to the allowed list.
 */
task('upgradeProxy', 'Upgrades the proxy to the new implementation')
  .addParam('proxyAddress', 'The address of the BFT proxy contract')
  .addParam('updatedContractAddress', 'The address of the updated contract')
  .addParam('updatedContractName', 'The updated contract name')
  .addParam('referenceContract', 'The reference contract name to be used')
  .setAction(
    async (
      {
        proxyAddress,
        updatedContractAddress,
        referenceContract,
        updatedContractName,
      },
      hre: HardhatRuntimeEnvironment,
    ): Promise<void> => {
      try {
        log('Starting proxy upgrade process...');

        await compileContracts(hre);

        await upgradeProxy(
          hre,
          referenceContract,
          proxyAddress,
          updatedContractAddress,
        );

        log('Proxy upgrade completed successfully.');
      } catch (error) {
        log(`Error during proxy upgrade: ${error}`);
        throw error;
      }
    },
  );
