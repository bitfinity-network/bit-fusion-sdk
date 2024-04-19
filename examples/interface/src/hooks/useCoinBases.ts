/* eslint-disable import/no-unused-modules */
import { Currency, Token } from '@uniswap/sdk-core'
import { useEffect, useState } from 'react'

import { SupportedChainId } from './../constants/chains'
import {
  CEUR_CELO,
  CEUR_CELO_ALFAJORES,
  CMC02_CELO,
  CUSD_CELO,
  CUSD_CELO_ALFAJORES,
  DAI,
  DAI_ARBITRUM_ONE,
  DAI_OPTIMISM,
  DAI_POLYGON,
  nativeOnChain,
  PORTAL_ETH_CELO,
  PORTAL_USDC_CELO,
  USDC_ARBITRUM,
  USDC_MAINNET,
  USDC_OPTIMISM,
  USDC_POLYGON,
  USDT,
  USDT_ARBITRUM_ONE,
  USDT_OPTIMISM,
  USDT_POLYGON,
  WBTC,
  WBTC_ARBITRUM_ONE,
  WBTC_OPTIMISM,
  WBTC_POLYGON,
  WETH_POLYGON,
  WETH_POLYGON_MUMBAI,
  WRAPPED_NATIVE_CURRENCY,
} from './../constants/tokens'
import useFaucet from './useFaucet'

/**
 * Shows up in the currency select for swap and add liquidity
 */
type ChainCurrencyList = {
  readonly [chainId: number]: Currency[]
}

export default function useCoinBasis() {
  const faucetTokens = useFaucet()
  const [coinBases, setCoinBases] = useState<ChainCurrencyList>()

  useEffect(() => {
    if (faucetTokens.length) {
      setCoinBases({
        [SupportedChainId.MAINNET]: [
          nativeOnChain(SupportedChainId.MAINNET),
          DAI,
          USDC_MAINNET,
          USDT,
          WBTC,
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.MAINNET] as Token,
        ],
        [SupportedChainId.BITFINITY]: [
          nativeOnChain(SupportedChainId.BITFINITY),
          ...faucetTokens,
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.BITFINITY] as Token,
        ],
        [SupportedChainId.ROPSTEN]: [
          nativeOnChain(SupportedChainId.ROPSTEN),
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.ROPSTEN] as Token,
        ],
        [SupportedChainId.RINKEBY]: [
          nativeOnChain(SupportedChainId.RINKEBY),
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.RINKEBY] as Token,
        ],
        [SupportedChainId.GOERLI]: [
          nativeOnChain(SupportedChainId.GOERLI),
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.GOERLI] as Token,
        ],
        [SupportedChainId.KOVAN]: [
          nativeOnChain(SupportedChainId.KOVAN),
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.KOVAN] as Token,
        ],
        [SupportedChainId.ARBITRUM_ONE]: [
          nativeOnChain(SupportedChainId.ARBITRUM_ONE),
          DAI_ARBITRUM_ONE,
          USDC_ARBITRUM,
          USDT_ARBITRUM_ONE,
          WBTC_ARBITRUM_ONE,
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.ARBITRUM_ONE] as Token,
        ],
        [SupportedChainId.ARBITRUM_RINKEBY]: [
          nativeOnChain(SupportedChainId.ARBITRUM_RINKEBY),
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.ARBITRUM_RINKEBY] as Token,
        ],
        [SupportedChainId.OPTIMISM]: [
          nativeOnChain(SupportedChainId.OPTIMISM),
          DAI_OPTIMISM,
          USDC_OPTIMISM,
          USDT_OPTIMISM,
          WBTC_OPTIMISM,
        ],
        [SupportedChainId.OPTIMISM_GOERLI]: [nativeOnChain(SupportedChainId.OPTIMISM_GOERLI)],
        [SupportedChainId.POLYGON]: [
          nativeOnChain(SupportedChainId.POLYGON),
          WETH_POLYGON,
          USDC_POLYGON,
          DAI_POLYGON,
          USDT_POLYGON,
          WBTC_POLYGON,
        ],
        [SupportedChainId.POLYGON_MUMBAI]: [
          nativeOnChain(SupportedChainId.POLYGON_MUMBAI),
          WRAPPED_NATIVE_CURRENCY[SupportedChainId.POLYGON_MUMBAI] as Token,
          WETH_POLYGON_MUMBAI,
        ],

        [SupportedChainId.CELO]: [
          nativeOnChain(SupportedChainId.CELO),
          CEUR_CELO,
          CUSD_CELO,
          PORTAL_ETH_CELO,
          PORTAL_USDC_CELO,
          CMC02_CELO,
        ],
        [SupportedChainId.CELO_ALFAJORES]: [
          nativeOnChain(SupportedChainId.CELO_ALFAJORES),
          CUSD_CELO_ALFAJORES,
          CEUR_CELO_ALFAJORES,
        ],
      })
    }
  }, [faucetTokens])

  return coinBases
}
