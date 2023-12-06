import { ethers } from "hardhat";
import { HardhatUserConfig } from "hardhat/config";
import "@nomicfoundation/hardhat-toolbox-viem";
const dotenv = require("dotenv");

dotenv.config();

// chain urls
const LOCAL_EVMC_URL = "http://127.0.0.1:8545";

const config: HardhatUserConfig = {
  solidity: {
    version: "0.8.19",
    settings: {
      optimizer: {
        runs: 200,
        enabled: true,
      },
    },
  },
  defaultNetwork: "evmc",
  networks: {
    evmc: {
      chainId: 355113,
      url: LOCAL_EVMC_URL,
      accounts: [...(process.env.PRIVATE_KEY ? [process.env.PRIVATE_KEY] : [])],
    },
  },
};

export default config;
