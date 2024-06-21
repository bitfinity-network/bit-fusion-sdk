import { task } from 'hardhat/config';
import { HardhatRuntimeEnvironment } from 'hardhat/types';


/**
 * Upgrades the BFT contract to a new implementation.
 *
 * @param proxyAddress - The address of the BFT proxy contract to be upgraded.
 * @param hre - The Hardhat Runtime Environment.
 * @returns The upgraded BFT contract.
 */
task('upgrade-bft', 'Upgrades the BFT contract')
    .addParam('proxyAddress', 'The address of the BFT proxy contract')
    .setAction(

        async ({ proxyAddress }, hre: HardhatRuntimeEnvironment) => {
            console.log('Compiling contract');
            await hre.run('compile');
            console.log('Contract compiled');

            const { network } = hre.hardhatArguments;

            if (!network) {
                throw new Error('Please specify a network');
            }

            // Validate the proxy address
            if (!hre.ethers.isAddress(proxyAddress)) {
                throw new Error(`Invalid proxy address: ${proxyAddress}`);
            }

            console.log('Upgrading BFT contract');

            const BFTBridge = await hre.ethers.getContractFactory('BFTBridge');

            console.log('Upgrading BFT contract');
            const upgradedBridge = await hre.upgrades.upgradeProxy(
                proxyAddress,
                BFTBridge
            );

            // Wait for the upgrade to be confirmed
            await upgradedBridge.waitForDeployment();

            // Get the new implementation address
            const newImplementationAddress =
                await hre.upgrades.erc1967.getImplementationAddress(proxyAddress);

            console.log(`BFT contract upgraded`);
            console.log(`Proxy address: ${proxyAddress}`);
            console.log(`New implementation address: ${newImplementationAddress}`);
        }
    );
