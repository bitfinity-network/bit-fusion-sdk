/* eslint-disable import/no-unused-modules */
import { Token } from "@uniswap/sdk-core";
import { TokenInfo } from "@uniswap/token-lists";
import { TokenContext } from "contexts/Token/context";
import { useContext, useEffect, useState } from "react";

import { SupportedChainId } from "./../constants/chains";

export default function useFaucet() {
  const tokens = useContext(TokenContext);
  const [faucetTokens, setFaucetTokens] = useState<Token[]>([]);

  useEffect(() => {
    if (tokens && Object.values(tokens).length) {
      setFaucetTokens(
        Object.values(tokens).map(
          (i: TokenInfo) =>
            new Token(
              SupportedChainId.BITFINITY,
              i.address,
              i.decimals,
              i.symbol,
              i.name
            )
        )
      );
    }
  }, [tokens]);

  return faucetTokens;
}
