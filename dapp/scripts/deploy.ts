import { formatEther, parseEther } from "viem";
import hre from "hardhat";

const ETHCONTRACT = "0xa2f96ef6ed3d67a0352e659b1e980f13e098618f"

async function main() {

  const wrapper = await hre.viem.deployContract("WERC20", [ETHCONTRACT]);

  console.log(
    `Wrapper fro ETH contract ${ETHCONTRACT} deployed to ${wrapper.address}`
  );
}
// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
