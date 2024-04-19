import { Token } from '@uniswap/sdk-core'
import React, { useCallback, useState } from 'react'
import { ChevronDown } from 'react-feather'

import {
  DropDownContainer,
  DropDownHeader,
  DropDownList,
  DropDownListContainer,
  ListItem,
} from './styled-faucet-components'

interface DropDownProps {
  currentToken: Token | undefined
  updateCurrentToken: React.Dispatch<React.SetStateAction<Token | undefined>>
  currentTokenAddress: string
  updateSelectedTokenAddress: React.Dispatch<React.SetStateAction<string | undefined>>
  availableTokens: Token[]
}

export default function FaucetDropDown({
  currentToken,
  updateCurrentToken,
  currentTokenAddress,
  updateSelectedTokenAddress,
  availableTokens,
}: DropDownProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [selectedOption, setSelectedOption] = useState<string | undefined>(currentToken?.name)

  const toggling = useCallback(() => setIsOpen((o) => !o), [])

  const onOptionClicked = useCallback(
    (value: Token) => {
      setSelectedOption(value.name)
      setIsOpen(false)
      updateCurrentToken(value)
      updateSelectedTokenAddress(value.address)
    },
    [updateCurrentToken, updateSelectedTokenAddress]
  )

  return (
    <DropDownContainer>
      <div style={{ width: '100%' }}>
        <DropDownHeader
          onClick={(e) => {
            // prevent page reload
            e.preventDefault()
            toggling()
          }}
        >
          <span
            id={currentTokenAddress}
            style={{ width: '100%', textAlign: 'left', padding: '8px', position: 'relative' }}
          >
            {selectedOption}
          </span>
          <ChevronDown size={15} />
        </DropDownHeader>
        {isOpen && (
          <DropDownListContainer>
            <DropDownList>
              {availableTokens.map((token: Token) => (
                <ListItem
                  onClick={(e) => {
                    // prevent page reload
                    e.preventDefault()
                    onOptionClicked(token)
                  }}
                  key={token.address}
                >
                  <span>{token.name}</span>
                </ListItem>
              ))}
            </DropDownList>
          </DropDownListContainer>
        )}
      </div>
    </DropDownContainer>
  )
}
