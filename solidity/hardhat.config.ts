
import { HardhatUserConfig } from "hardhat/config";
import "@nomicfoundation/hardhat-toolbox";
import "@nomicfoundation/hardhat-foundry";
import "@openzeppelin/hardhat-upgrades";
import * as dotenv from 'dotenv';
dotenv.config();


/// Tasks that are used to interact with the BFT contract
import "./tasks/deploy-bft";
import "./tasks/fee-charge-address";
import "./tasks/deploy-fee-charge";
import "./tasks/upgrade-bft";
import "./tasks/pause-unpause";


const MAINNET_URL = "https://mainnet.bitfinity.network"

const TESTNET_URL = "https://testnet.bitfinity.network"

const DEPLOYER_PRIVATE_KEY = process.env.PRIVATE_KEY || "";


const config: HardhatUserConfig = {
  networks: {
    localhost: {
      url: "http://127.0.0.1:8545",
      accounts: [`0x${DEPLOYER_PRIVATE_KEY}`]
    },
    mainnet: {
      url: MAINNET_URL,
      accounts: [`0x${DEPLOYER_PRIVATE_KEY}`]
    },
    testnet: {
      url: TESTNET_URL,
      accounts: [`0x${DEPLOYER_PRIVATE_KEY}`]
    }
  },
  solidity: {
    version: '0.8.25',
    settings: {
      optimizer: {
        enabled: true,
        runs: 200
      },
      outputSelection: {
        '*': {
          '*': ['storageLayout'],
        },
      },
    }
  }
}



export default config;
