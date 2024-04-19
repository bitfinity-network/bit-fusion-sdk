import * as dotenv from 'dotenv';

import { HardhatUserConfig } from 'hardhat/config';
import '@nomiclabs/hardhat-etherscan';
import '@nomiclabs/hardhat-waffle';
import '@typechain/hardhat';

dotenv.config();

// You need to export an object to set up your config
// Go to https://hardhat.org/config/ to learn more

// chain urls
const LOCAL = 'http://127.0.0.1:8545';

const TESTNET = 'https://4fe7g-7iaaa-aaaak-aegcq-cai.raw.ic0.app';

const MAINNET =
  'https://i3jjb-wqaaa-aaaaa-qadrq-cai.raw.ic0.app';

const DEVNET = 'https://emz6j-kiaaa-aaaak-ae35a-cai.raw.ic0.app';

const config: HardhatUserConfig = {
  solidity: {
    version: '0.7.6',
    settings: {
      optimizer: {
        runs: 200,
        enabled: true,
      },

    },
  },
  defaultNetwork: 'testnet',
  networks: {
    localhost: {},
    testnet: {
      url: TESTNET,
      accounts: [
        ...(process.env.UNISWAP_DEPLOYER ? [process.env.UNISWAP_DEPLOYER] : []),
      ],
    },
    mainnet: {
      url: MAINNET,
      accounts: [
        ...(process.env.UNISWAP_DEPLOYER ? [process.env.UNISWAP_DEPLOYER] : []),
      ],
    },
    devnet: {
      url: DEVNET,
      accounts: [
        ...(process.env.UNISWAP_DEPLOYER ? [process.env.UNISWAP_DEPLOYER] : []),
      ],
    },
    local: {
      url: LOCAL,
      accounts: [
        ...(process.env.UNISWAP_DEPLOYER ? [process.env.UNISWAP_DEPLOYER] : []),
      ]
    }
  },
};

export default config;