import { task } from "hardhat/config";
import { boolean, int } from "hardhat/internal/core/params/argumentTypes";

/// A task for computing the fee charge address
task("fee-charge-address", "Computes the fee charge address")
  .addParam("nonce", "The nonce of the transaction", undefined, int)
  .addOptionalParam("deployerAddress", "The address of the deployer")
  .setAction(async ({ nonce, deployerAddress }, hre) => {
    if (deployerAddress === undefined) {
      let [deployer] = await hre.ethers.getSigners();
      deployerAddress = deployer.address;
    }

    const feeChargeAddress = hre.ethers.getCreateAddress({ from: deployerAddress, nonce });

    console.log(`Fee charge address: ${feeChargeAddress}`);
  });
