import { createContext, ReactNode, useEffect, useState } from 'react';

import { Tokens, TokensClass } from './../../constants/contracts';

export const TokenContext = createContext<TokensClass | undefined>(undefined);

import { tokens as tokenAddresses } from '../../contracts/tokenAddresses.json';

export function TokenProvider({ children }: { children: ReactNode }) {
  const [tokens, setTokens] = useState<TokensClass>();

  useEffect(() => {
    (async () => {
      const tokens: TokensClass = tokenAddresses;
      setTokens(tokens);
    })();
  }, []);

  return (
    <TokenContext.Provider value={tokens}>{children}</TokenContext.Provider>
  );
}
