import contracts from "../contracts/contractAddresses.json"
import tokenAddress from "../contracts/tokenAddresses.json"

// interface Contracts {
//   date: Date
//   contracts: ContractsClass
// }

// interface ContractsClass {
//   weth9: Factory
//   factory: Factory
//   nftDescriptorLibrary: Factory
//   nftDescriptor: Factory
//   positionManager: Factory
//   router: Factory
//   multicall: Factory
//   quoter: Factory
//   migrator: Factory
//   ticklens: Factory
// }
//
// interface Factory {
//   address: string
// }

export interface Tokens {
  date: Date
  tokens: TokensClass
}

export interface TokensClass {
  Cashium: TokenInfo
  Coinaro: TokenInfo
  Coinverse: TokenInfo
  Coinovation: TokenInfo
  Intellicoin: TokenInfo
  Incoingnito: TokenInfo
  Arcoin: TokenInfo
  Coinicious: TokenInfo
}

interface TokenInfo {
  address: string
  name: string
  symbol: string
  decimals: number
}


const getContractAddresses = () => {
  const contractAddresses: { [key: string]: string } = {};

  Object.entries(contracts).forEach(([key, value]: any) => {
    if (typeof value === 'object' && value !== null) {
      contractAddresses[key.toUpperCase()] = value.address;
    }
  });

  localStorage.setItem('CONTRACT_ADDRESSES', JSON.stringify(contractAddresses));

  console.log('Saved contract addresses to localStorage')
}

const getTokens = () => {
  const tokens: { [key: string]: TokenInfo } = {};

  Object.entries(tokenAddress).forEach(([key, value]: any) => {
    if (typeof value === 'object' && value !== null) {
      tokens[key] = {
        address: value.address,
        name: value.name,
        symbol: value.symbol,
        decimals: value.decimals
      }
    }
  });

  localStorage.setItem('TOKENS', JSON.stringify(tokens));

  console.log('Saved tokens to localStorage')
}



const initContracts = () => {
  try {
    getContractAddresses()
    getTokens()
  } catch (e) {
    console.log(e)
  }
}


initContracts();

export default initContracts
