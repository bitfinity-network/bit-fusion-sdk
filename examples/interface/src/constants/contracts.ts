// const CONTRACT_GCS_FILE_PATH = 'https://evmc.storage.googleapis.com/Addresses/logs/contractAddresses.json'

import { contracts as contractAddresses } from "../contracts/contractAddresses.json"

interface Contracts {
  date: Date
  contracts: ContractsClass
}

interface ContractsClass {
  weth9: Factory
  factory: Factory
  nftDescriptorLibrary: Factory
  nftDescriptor: Factory
  positionManager: Factory
  router: Factory
  multicall: Factory
  quoter: Factory
  migrator: Factory
  ticklens: Factory
}

interface Factory {
  address: string
}

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

const getContractAddresses = async () => {
  const CONTRACTS_ADDRESSES: { [key: string]: string } = {}



  const contracts: ContractsClass = contractAddresses
  const contractNames: string[] = Object.keys(contracts)

  contractNames.forEach((contractName: string) => {
    CONTRACTS_ADDRESSES[contractName.toUpperCase()] =
      // @ts-ignore
      contracts[contractName].address
  })

  localStorage.setItem('CONTRACTS_ADDRESSES', JSON.stringify(CONTRACTS_ADDRESSES))
}

const initContracts = async () => {
  try {
    await getContractAddresses()
  } catch (e) {
    console.log(e)
  }
}

initContracts().catch(console.error)

export default initContracts
