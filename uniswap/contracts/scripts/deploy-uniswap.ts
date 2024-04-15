// We require the Hardhat Runtime Environment explicitly here. This is optional
// but useful for running the script in a standalone fashion through `node <script>`.
//
// When running the script with `npx hardhat run <script>` you'll find the Hardhat
// Runtime Environment's members available in the global scope.
import { ethers } from "hardhat";
import { ContractFactory } from "ethers";


import WETH9 from "../utils/weth9.json";
import FACTORY from "@uniswap/v3-core/artifacts/contracts/UniswapV3Factory.sol/UniswapV3Factory.json";
import {
  abi as SWAP_ROUTER_ABI,
  bytecode as SWAP_ROUTER_BYTECODE,
} from "@uniswap/swap-router-contracts/artifacts/contracts/SwapRouter02.sol/SwapRouter02.json";
import NFT_DESCRIPTOR from "@uniswap/v3-periphery/artifacts/contracts/libraries/NFTDescriptor.sol/NFTDescriptor.json";
import POSITION_MANAGER from "@uniswap/v3-periphery/artifacts/contracts/NonfungiblePositionManager.sol/NonfungiblePositionManager.json";
import QUOTER from "@uniswap/v3-periphery/artifacts/contracts/lens/Quoter.sol/Quoter.json";
import MIGRATOR from "@uniswap/v3-periphery/artifacts/contracts/V3Migrator.sol/V3Migrator.json";
import TICKLENS from "@uniswap/v3-periphery/artifacts/contracts/lens/TickLens.sol/TickLens.json";
import MULTICALL from "@uniswap/v3-periphery/artifacts/contracts/lens/UniswapInterfaceMulticall.sol/UniswapInterfaceMulticall.json";

import writeLogFile from "../helpers/write-files";
import getNetworkInfo from "../helpers/get-network-info";
import Web3 from "web3";

async function main() {
  const addresses: any = {};

  const actors = await ethers.getSigners();
  const deployer = actors[0];
  const networkID = await getNetworkInfo(deployer);


  if (networkID === 355113) {
    const res = await ethers.provider.send("ic_mintNativeToken", [
      deployer.address,
      "0x21e19e0c9bab2400000",
    ]);
    console.log("minted EVMC token: ", Web3.utils.hexToNumberString(res));
  }

  const Weth9 = new ContractFactory(
    WETH9.abi,
    WETH9.bytecode,
    deployer
  );
  const weth9Deploy = Weth9.getDeployTransaction();
  const wethTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...weth9Deploy,
  });

  const weth9 = await wethTx.wait();
  addresses.weth9 = { address: weth9.contractAddress };
  console.log("WETH9: ", weth9.contractAddress);

  const Factory = new ContractFactory(
    FACTORY.abi,
    FACTORY.bytecode,
    deployer,
  );

  const factoryDeploy = Factory.getDeployTransaction();
  const factoryTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...factoryDeploy,
  });

  const factory = await factoryTx.wait();
  addresses.factory = { address: factory.contractAddress };
  console.log("Factory: ", factory.contractAddress);

  const NftDescriptorLibrary = new ContractFactory(
    NFT_DESCRIPTOR.abi,
    NFT_DESCRIPTOR.bytecode,
    deployer,
  );

  const nftDescriptorLibraryDeploy =
    NftDescriptorLibrary.getDeployTransaction();
  const nftDescriptorLibraryTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...nftDescriptorLibraryDeploy,
  });

  const nftDescriptorLibrary = await nftDescriptorLibraryTx.wait();
  addresses.nftDescriptorLibrary = {
    address: nftDescriptorLibrary.contractAddress,
  };
  console.log("NFT Descriptor Library: ", nftDescriptorLibrary.contractAddress);

  const positionDescriptorFactory = await ethers.getContractFactory(
    "NonfungibleTokenPositionDescriptor",
    {
      libraries: {
        NFTDescriptor: nftDescriptorLibrary.contractAddress,
      },
    }
  );
  const nftDescriptorDeploy = positionDescriptorFactory.getDeployTransaction(
    weth9.contractAddress,
    // 'ETH' as a bytes32 string
    ethers.utils.formatBytes32String("ETH")
  );

  const nftDescriptorTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...nftDescriptorDeploy,
  });

  const nftDescriptor = await nftDescriptorTx.wait();
  addresses.nftDescriptor = { address: nftDescriptor.contractAddress };
  console.log("NFT Descriptor: ", nftDescriptor.contractAddress);

  const PositionManager = new ContractFactory(
    POSITION_MANAGER.abi,
    POSITION_MANAGER.bytecode,
    deployer,
  );

  const positionManagerDeploy = PositionManager.getDeployTransaction(
    factory.contractAddress,
    weth9.contractAddress,
    nftDescriptor.contractAddress
  );
  const positionManagerTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...positionManagerDeploy,
  });
  const positionManager = await positionManagerTx.wait();

  addresses.positionManager = { address: positionManager.contractAddress };
  console.log("Position Manager: ", positionManager.contractAddress);

  const Router = new ContractFactory(
    SWAP_ROUTER_ABI,
    SWAP_ROUTER_BYTECODE,
    deployer,
  );
  const routerDeploy = Router.getDeployTransaction(
    factory.contractAddress,
    factory.contractAddress,
    positionManager.contractAddress,
    weth9.contractAddress
  );
  const routerTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...routerDeploy,
  });
  const router = await routerTx.wait();

  addresses.router = { address: router.contractAddress };
  console.log("Router: ", router.contractAddress);

  const Multicall = new ContractFactory(
    MULTICALL.abi,
    MULTICALL.bytecode,
    deployer,
  );
  const multicallDeploy = Multicall.getDeployTransaction();
  const multicallTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...multicallDeploy,
  });
  const multicall = await multicallTx.wait();

  addresses.multicall = { address: multicall.contractAddress };
  console.log("Multicall: ", multicall.contractAddress);

  const Quoter = new ContractFactory(
    QUOTER.abi,
    QUOTER.bytecode,
    deployer
  );
  const quoterDeploy = Quoter.getDeployTransaction(
    factory.contractAddress,
    weth9.contractAddress
  );
  const quoterTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...quoterDeploy,
  });
  const quoter = await quoterTx.wait();

  addresses.quoter = { address: quoter.contractAddress };
  console.log("Quoter: ", quoter.contractAddress);

  const Migrator = new ContractFactory(
    MIGRATOR.abi,
    MIGRATOR.bytecode,
    deployer
  );
  const migratorDeploy = Migrator.getDeployTransaction(
    factory.contractAddress,
    weth9.contractAddress,
    positionManager.contractAddress
  );
  const migratorTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...migratorDeploy,
  });
  const migrator = await migratorTx.wait();
  addresses.migrator = { address: migrator.contractAddress };
  console.log("Migrator: ", migrator.contractAddress);

  const TickLens = new ContractFactory(
    TICKLENS.abi,
    TICKLENS.bytecode,
    deployer
  );

  const ticklensDeploy = TickLens.getDeployTransaction();
  const ticklensTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...ticklensDeploy,
  });
  const ticklens = await ticklensTx.wait();
  addresses.ticklens = { address: ticklens.contractAddress };
  console.log("TickLens: ", ticklens.contractAddress);

  // write file to json
  const input = { date: new Date(), contracts: addresses };
  const file: string = "contractAddresses.json";
  writeLogFile(file, input);
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
