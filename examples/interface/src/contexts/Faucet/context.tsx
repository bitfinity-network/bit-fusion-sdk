import { createContext, ReactNode, useEffect, useState } from 'react'

export const FaucetContext = createContext<string>('')

interface Faucet {
  date: Date
  faucetAddress: string
}

// const FAUCET_GCS_FILE_PATH = 'https://evmc.storage.googleapis.com/Addresses/logs/faucetAddress.json'

import {faucetAddress as faucetAddresses} from "../../constants/contracts/faucetAddresses.json"

export function FaucetProvider({ children }: { children: ReactNode }) {
  const [faucets, setFaucets] = useState<string>('')

  useEffect(() => {
    ;(async () => {
      const faucetAddress: string = faucetAddresses
      setFaucets(faucetAddress)
    })()
  })

  return <FaucetContext.Provider value={faucets}>{children}</FaucetContext.Provider>
}
