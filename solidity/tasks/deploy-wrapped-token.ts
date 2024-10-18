/// A task for deploying the fee charge contract

import { task } from 'hardhat/config';

task(
  'deploy-wrapped-token',
  'Deploys a wrapped token on the WrappedTokenDeployer contract',
)
  .addParam(
    'wrappedTokenDeployer',
    'The addresses of the wrapped token deployer',
  )
  .addParam('name', 'The name of the token')
  .addParam('symbol', 'The symbol of the token')
  .addParam('decimals', 'The decimals of the token')
  .addParam('owner', 'The address of the token owner')
  .setAction(
    async ({ wrappedTokenDeployer, name, symbol, decimals, owner }, hre) => {
      const { network } = hre.hardhatArguments;

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

      const [deployer] = await hre.ethers.getSigners();
      const contract = await hre.ethers.getContractAt(
        'WrappedTokenDeployer',
        wrappedTokenDeployer,
        deployer,
      );

      // deploy erc20
      const response = await contract.deployERC20(
        name,
        symbol,
        decimals,
        owner,
      );
      const receipt = await response.wait();
      // Make sure the receipt status is 1

      if (!receipt || receipt.status !== 1) {
        throw new Error('Failed to deploy ERC20');
      }

      // get event
      const event = receipt.logs
        .map((log) => contract.interface.parseLog(log))
        .filter((maybeLog) => maybeLog !== null)
        .find((parsedLog) => parsedLog.name === 'ERC20Deployed');

      if (!event) {
        throw new Error('Failed to get ERC20Deployed event');
      }

      const tokenAddress = event.args[0];
      console.log('ERC20 deployed at:', tokenAddress);
    },
  );
