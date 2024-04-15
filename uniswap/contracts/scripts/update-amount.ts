// We require the Hardhat Runtime Environment explicitly here. This is optional
// but useful for running the script in a standalone fashion through `node <script>`.
//
// When running the script with `npx hardhat run <script>` you'll find the Hardhat
// Runtime Environment's members available in the global scope.
import { ethers } from 'hardhat';
import faucetJSON from '../artifacts/contracts/Faucet.sol/Faucet.json';
import { BigNumber } from 'ethers';


async function main() {
  const senders = await ethers.getSigners();
  // choose other address than deployer
  const owner = senders[0];
  const sender = senders[1];



  // define the new amount that the faucet will spend when tokens are claimed
  var newAmount = ethers.utils.formatUnits(100000000000, 8);
  const bigNumberNewAmount: BigNumber = BigNumber.from(
    ethers.utils.parseUnits(newAmount)
  );

  const faucetAddress = '0xa57ea1539e797637Da1923039EC91a2bbb702a6F';
  const faucetABI = faucetJSON.abi;

  // get faucet Contract Instance with contract owner
  const ownerFaucetContractAddress = await ethers.getContractAt(
    faucetABI,
    faucetAddress,
    owner
  );

  // get faucet Contract Instance with another signer
  const senderFaucetContractInstance = await ethers.getContractAt(
    faucetABI,
    faucetAddress,
    sender
  );
  // get current claimable amount
  const currentAmount = await senderFaucetContractInstance.getAmount();
  console.log(
    `current Amount that can be claimed ${ethers.utils.formatUnits(
      currentAmount,
      18
    )} tokens`
  );

  const approve_tx =
    await ownerFaucetContractAddress.populateTransaction.setAmount(
      bigNumberNewAmount
    );

  await owner.sendTransaction({
    nonce: await owner.getTransactionCount(),
    ...approve_tx,
  });

  ownerFaucetContractAddress.setAmount(bigNumberNewAmount);

  // get updated claimable amount
  const updatedAmount = await senderFaucetContractInstance.getAmount();
  console.log(
    `new Amount that can be claimed ${ethers.utils.formatEther(
      updatedAmount
    )} tokens`
  );
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
