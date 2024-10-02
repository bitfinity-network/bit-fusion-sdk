import { task } from "hardhat/config";
import { boolean } from "hardhat/internal/core/params/argumentTypes";
import { HardhatRuntimeEnvironment } from "hardhat/types";

/// Deploys the BFT contract using the provided parameters.
///
/// @param minterAddress The address of the minter.
/// @param feeChargeAddress The address of the fee charge.
/// @param isWrappedSide A boolean indicating whether this is the wrapped side.
///
/// This task will:
/// 1. Compile the contract.
/// 2. Validate the provided addresses.
/// 3. Deploy the BFT contract using the provided parameters.
/// 4. Wait for the deployment to be confirmed.
/// 5. Log the deployed proxy address and implementation address.

task("deploy-bft", "Deploys the BFT contract")
  .addParam("minterAddress", "The address of the minter")
  .addParam("feeChargeAddress", "The address of the fee charge")
  .addParam("wrappedTokenDeployerAddress", "The address of the wrapped token deployer")
  .addParam("isWrappedSide", "Is the wrapped side", undefined, boolean)
  .addOptionalParam("owner", "The owner of the contract")
  .addOptionalParam("controllers", "The controllers of the contract")
  .setAction(
    async (
      { minterAddress, feeChargeAddress, wrappedTokenDeployerAddress, isWrappedSide, owner, controllers },
      hre: HardhatRuntimeEnvironment,
    ) => {
      console.log("Compiling contract");
      await hre.run("compile");
      console.log("Contract compiled");

      let controllersArr = controllers ? controllers.split(",") : [];
      let ownerAddress = owner || hre.ethers.ZeroAddress;

      // Validate the arguments that it are addresses
      const addressesToValidate = [minterAddress, feeChargeAddress, wrappedTokenDeployerAddress];
      if (owner) addressesToValidate.push(ownerAddress);
      if (controllers) addressesToValidate.push(...controllersArr);

      // Validate the arguments that it is address
      for (const address of addressesToValidate) {
        if (!hre.ethers.isAddress(address)) {
          throw new Error(`Invalid address: ${address}`);
        }
      }

      console.log("Deploying BFT contract");
      const { network } = hre.hardhatArguments;

      if (!network) {
        throw new Error("Please specify a network");
      }

      const BFTBridge = await hre.ethers.getContractFactory("BFTBridge");


      const bridge = await hre.upgrades.deployProxy(BFTBridge, [
        minterAddress,
        feeChargeAddress,
        wrappedTokenDeployerAddress,
        isWrappedSide,
        ownerAddress,
        controllersArr,
      ]);

      // Wait for the deployment to be confirmed
      await bridge.waitForDeployment();

      // Get the address of the proxy
      const proxyAddress = await bridge.getAddress();

      // Get implementation address
      const implementationAddress = await hre.upgrades.erc1967.getImplementationAddress(proxyAddress);

      console.log(`BFT deployed to: ${proxyAddress}`);
      console.log(`Implementation deployed to: ${implementationAddress}`);
    },
  );
