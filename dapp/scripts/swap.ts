

import { ethers } from "ethers";
import IERC20Artifact from "../artifacts/@openzeppelin/contracts/token/ERC20/IERC20.sol/IERC20.json";


async function main() {
  const provider = new ethers.JsonRpcProvider();
  const signer = await provider.getSigner();
  const IERC20 = IERC20Artifact.abi
  const tokenAddress = "0xa2f96ef6ed3d67a0352e659b1e980f13e098618f"; // replace with your token address
  const recipient = "0xRecipientAddress"; // replace with recipient address
  const amount = ethers.parseUnits("10.0", 18); // replace "10.0" with the amount you want to send

  const tokenContract = new ethers.Contract(tokenAddress, IERC20, provider);

  async function approveAndTransfer() {
    try {
      const approveTx = await tokenContract.approve(recipient, amount);
      await approveTx.wait();

      const transferTx = await tokenContract.transferFrom(signer.getAddress(), recipient, amount);
      await transferTx.wait();

      console.log(`Transferred ${amount} tokens from ${signer.getAddress()} to ${recipient}`);
    } catch (error) {
      console.error(`Failed to transfer tokens: ${error}`);
    }
  }

  approveAndTransfer();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
