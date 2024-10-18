/// A task for deploying the fee charge contract

import { task } from 'hardhat/config';

task('deploy-fee-charge', 'Deploys the fee charge contract')
  .addParam('bridges', 'The addresses of the bridges')
  .addOptionalParam('expectedAddress', 'The expected address of the fee charge')
  .setAction(async ({ bridges, expectedAddress }, hre) => {
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

    let bridgesArr: string[] = bridges ? bridges.split(',') : [];

    if (bridgesArr.length === 0) {
      throw new Error('Bridges must be a non-empty array of addresses');
    }

    // Validate the arguments that it is address
    for (const address of bridgesArr) {
      if (!hre.ethers.isAddress(address)) {
        throw new Error(`Invalid address: ${address}`);
      }
    }

    const FeeCharge = await hre.ethers.getContractFactory('FeeCharge');
    const feeCharge = await FeeCharge.deploy(bridgesArr);

    // Wait for the deployment to be confirmed
    await feeCharge.waitForDeployment();

    // Get Address
    const feeChargeAddress = await feeCharge.getAddress();

    // Check if the fee charge address is as expected
    if (
      expectedAddress !== undefined &&
      feeChargeAddress.toLowerCase() !== expectedAddress.toLowerCase()
    ) {
      console.error(
        `Expected Address: ${expectedAddress} but got ${feeChargeAddress}`,
      );

      throw new Error('Fee charge address does not match the expected address');
    }

    console.log(`Fee charge address: ${feeChargeAddress}`);
  });
