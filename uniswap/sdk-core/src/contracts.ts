const tokenAddress = {"date":"2024-04-16T10:50:49.834Z","tokens":{"Cashium":{"address":"0x6ef0184AEF0De2E176B30BEb79ebD1d6be6B1B62","name":"Cashium","symbol":"CSM","decimals":18},"Coinaro":{"address":"0x2CF1dF9aB7b0467be009e13Fee94f6Dfcd160fB1","name":"Coinaro","symbol":"CNR","decimals":18},"Coinverse":{"address":"0x048B0c91C15d7b11ab9Bc2bb4B8368d4ddD155F3","name":"Coinverse","symbol":"CVS","decimals":18},"Coinovation":{"address":"0xC6Ad32B924927d5071BE7c16dA0F1Eb0c42699F6","name":"Coinovation","symbol":"COV","decimals":18},"Intellicoin":{"address":"0x87ccaC5BDf6cb5A1704142aFA1696ed2F51688D0","name":"Intellicoin","symbol":"ITC","decimals":18},"Incoingnito":{"address":"0xA13AC1Af146DF4A4502700253a94C2F1844eEDA5","name":"Incoingnito","symbol":"ICG","decimals":18},"Arcoin":{"address":"0xD33aEd3f0E79Ac7CA29c13e39F33B1B61b6e4313","name":"Arcoin","symbol":"ARC","decimals":18},"Coinicious":{"address":"0x56d8D4f7d99CFf9692082984E624d1E8D207ed04","name":"Coinicious","symbol":"CNS","decimals":18}}}

const contracts = {
  "date": "2024-04-16T10:47:29.838Z",
  "contracts": {
    "weth9": {
      "address": "0xa87a02F12571Dd0B972Ea1A84f4F8961e60bE684"
    },
    "factory": {
      "address": "0x4b36116855a0aA9F70C9EB646C4b23860231b5A6"
    },
    "nftDescriptorLibrary": {
      "address": "0x87D8D72876fCaA4580B961563c2E6802371bd56F"
    },
    "nftDescriptor": {
      "address": "0xAD662954110B46Cb91dD83c664C0f9983D2fA75c"
    },
    "positionManager": {
      "address": "0xa57ea1539e797637Da1923039EC91a2bbb702a6F"
    },
    "router": {
      "address": "0x53611Ab037767647498EBd555C4CC339db4Ee617"
    },
    "multicall": {
      "address": "0x7f330e8f7628a86b87B1c362cbFe18738DcfFf1c"
    },
    "quoter": {
      "address": "0x47C2579103f8ecD8cf0fF128869CdCF838352177"
    },
    "migrator": {
      "address": "0x3a961f066881D5fEfD3e8f9C2Fa9312e9DcfaDE1"
    },
    "ticklens": {
      "address": "0x582bF0F5B6fdb9EA788a4Fc211d83b0aC6d5132b"
    }
  }
}
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



export const initContracts = () => {
  try {
    getContractAddresses()
    getTokens()
  } catch (e) {
    console.log(e)
  }
}





