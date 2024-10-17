/// A task for deploying the fee charge contract

import { task } from 'hardhat/config';

task(
  'deploy-wrapped-token-deployer',
  'Deploys the WrappedTokenDeployer contract',
).setAction(async ({}, hre) => {
  const { network } = hre.hardhatArguments;

  console.log('Compiling contract');
  await hre.run('compile');
  console.log('Contract compiled');

  if (!network) {
    throw new Error('Please specify a network');
  }

  if (
    network === 'localhost' &&
    process.env.LOCALHOST_URL &&
    'url' in hre.network.config
  ) {
    hre.network.config.url = process.env.LOCALHOST_URL;
  }

  const WrappedTokenDeployer = await hre.ethers.getContractFactory(
    'WrappedTokenDeployer',
  );
  const wrappedTokenDeployer = await WrappedTokenDeployer.deploy();

  // Wait for the deployment to be confirmed
  await wrappedTokenDeployer.waitForDeployment();

  // Get Address
  const wrappedTokenDeployerAddress = await wrappedTokenDeployer.getAddress();

  console.log(`WrappedTokenDeployer address: ${wrappedTokenDeployerAddress}`);
});
