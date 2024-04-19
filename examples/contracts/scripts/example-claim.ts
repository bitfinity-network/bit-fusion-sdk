// We require the Hardhat Runtime Environment explicitly here. This is optional
// but useful for running the script in a standalone fashion through `node <script>`.
//
// When running the script with `npx hardhat run <script>` you'll find the Hardhat
// Runtime Environment's members available in the global scope.
import { NonceManager } from '@ethersproject/experimental';
import { ethers } from 'hardhat';
import faucetJSON from '../artifacts/contracts/Faucet.sol/Faucet.json';

async function main() {
  /*
  INFO: adapt PK in you .env file if you want to example claim with a different account than the deployer of the contracts
  --> then run: hardhat run scripts/example-claim.ts
   */
  const senders = await ethers.getSigners();

  // choose other address than deployer
  const sender = senders[1];

  const signedManager = new NonceManager(sender);
  // has to be a deployed faucet contract address on the CORRECT NETWORK
  // Ganache Faucet Address
  const faucetAddress = '0xa57ea1539e797637Da1923039EC91a2bbb702a6F';

  const faucetABI = faucetJSON.abi;
  // has to be a deployed token address on the CORRECT NETWORK
  const deployedTokenAddress = '0x2CF1dF9aB7b0467be009e13Fee94f6Dfcd160fB1';

  const faucetContractInstance = await ethers.getContractAt(
    faucetABI,
    faucetAddress,
    signedManager.signer
  );

  const approveTx = await faucetContractInstance.populateTransaction.claim(
    deployedTokenAddress
  );

  const claim = await signedManager.sendTransaction({
    nonce: await signedManager.getTransactionCount(),
    gasLimit: 9000000,
    ...approveTx,
  });

  await claim.wait();

  console.log('claiming tokens', claim);
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
