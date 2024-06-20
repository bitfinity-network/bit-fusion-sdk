import { task } from 'hardhat/config';
import { boolean } from 'hardhat/internal/core/params/argumentTypes';
import { HardhatRuntimeEnvironment } from 'hardhat/types';

/// Create a task for deployment of the BFT contract
task('deploy-bft', 'Deploys the BFT contract')
    .addParam('minterAddress', 'The address of the minter')
    .addParam('feeChargeAddress', 'The address of the fee charge')
    .addParam('isWrappedSide', 'Is the wrapped side', undefined, boolean)
    .setAction(
        async (
            { minterAddress, feeChargeAddress, isWrappedSide },
            hre: HardhatRuntimeEnvironment
        ) => {
            // Compile the contracts
            await hre.run('compile');

            // Validate the arguments that it is address
            for (const address of [minterAddress, feeChargeAddress]) {
                if (!hre.ethers.isAddress(address)) {
                    throw new Error(`Invalid address: ${address}`);
                }
            }

            console.log('Deploying BFT contract');

            const [deployer] = await hre.ethers.getSigners();
            const { network } = hre.hardhatArguments;

            if (!network) {
                throw new Error('Please specify a network');
            }

            const BFTBridge = await hre.ethers.getContractFactory('BFTBridge');

            const bridge = await hre.upgrades.deployProxy(
                BFTBridge,
                [minterAddress, feeChargeAddress, isWrappedSide],
                {
                    txOverrides: {
                        nonce: await deployer.getNonce('pending'),
                    },
                }
            );
            // Wait for the deployment to be confirmed
            await bridge.waitForDeployment();

            // Get the address of the proxy
            const proxyAddress = await bridge.getAddress();

            // Get implementation address
            const implementationAddress =
                await hre.upgrades.erc1967.getImplementationAddress(proxyAddress);

            console.log(`BFT deployed to: ${proxyAddress}`);
            console.log(`Implementation deployed to: ${implementationAddress}`);
        }
    );
