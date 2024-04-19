import { Trans } from '@lingui/macro'
import { Token } from '@uniswap/sdk-core'
import { useWeb3React } from '@web3-react/core'
import useFaucet from 'hooks/useFaucet'
import useMint from 'hooks/useMint'
import { useSingleCallResult } from 'lib/hooks/multicall'
import { useCallback, useEffect, useState } from 'react'
import { useToggleWalletModal } from 'state/application/hooks'

import { ButtonSecondary } from '../../components/Button'
import { ColumnCenter } from '../../components/Column'
import FaucetDropDown from '../../components/faucet/FaucetDropDown'
import FaucetTokenAddressPanel from '../../components/faucet/FaucetTokenAddressPanel'
import {
  FaucetContainer,
  FaucetInput,
  FaucetParent,
  Feedback,
  Form,
  FormWrapper,
  NativeParent,
  TitleRow,
  Wrapper,
} from '../../components/faucet/styled-faucet-components'
import { SupportedChainId } from '../../constants/chains'
import { useFaucetContract } from '../../hooks/useContract'
import { ThemedText } from '../../theme'

/*
IMPORT TOKENS
 */
export default function Faucet() {
  const faucetContract = useFaucetContract()
  const faucetTokens = useFaucet()
  const [claimable, setClaimable] = useState<boolean>(true)
  const [claimFeedback, setClaimFeedback] = useState<string>('')
  const [selectedToken, setSelectedToken] = useState<Token | undefined>()
  const [selectedTokenAddress, setSelectedTokenAddress] = useState<string>()
  const faucetState = useSingleCallResult(faucetContract, 'claim', [selectedTokenAddress])
  const { chainId, account } = useWeb3React()
  const isBITFINITYNetwork = chainId === SupportedChainId.BITFINITY
  const toggleWalletModal = useToggleWalletModal()
  const mintMutation = useMint()

  useEffect(() => {
    if (faucetTokens.length) {
      setSelectedToken(faucetTokens[0])
      setSelectedTokenAddress(faucetTokens[0].address)
    }
  }, [faucetTokens])
  /*
  handle request timeout of 60 seconds
   */
  const updateClaimTimeout = useCallback(() => {
    setClaimable(false)
    setTimeout(() => {
      setClaimable(true)
      setClaimFeedback('')
    }, 60000)
  }, [])

  /*
  handle token claim
   */
  const onClaimToken = useCallback(async () => {
    if (faucetContract && faucetState.valid && selectedTokenAddress) {
      try {
        const claim = await faucetContract.claim(selectedTokenAddress)
        updateClaimTimeout()
        setClaimFeedback(
          `Your transaction has been submitted. This can take a moment. Afterwards, check your Meta Mask for more infos.
           \n TX Hash: ${claim.hash}`
        )
      } catch (e) {
        try {
          if (e.code === 4001) {
            setClaimFeedback(e.message)
          } else if (e.data.message === 'execution reverted: Faucet Timeout Limit: Try again later') {
            console.log('error', e)
          } else {
            console.debug('Error:', e)
            setClaimFeedback(`Error: ${e.message}`)
          }
        } catch (e) {
          setClaimFeedback(`Unexpected error: ${e}`)
        }
      }
    } else {
      setClaimFeedback(`Something went wrong!`)
    }
  }, [faucetContract, faucetState.valid, selectedTokenAddress, updateClaimTimeout])

  return (
    <>
      <Wrapper>
        <ColumnCenter style={{ justifyContent: 'center' }}>
          <TitleRow
            style={{
              marginTop: '1rem',
              justifyContent: 'center',
              marginBottom: '2rem',
            }}
            padding="0"
          >
            <ThemedText.BodyPrimary fontSize="20px" style={{ justifyContent: 'center' }}>
              <Trans>Ethereum Faucet</Trans>
            </ThemedText.BodyPrimary>
          </TitleRow>

          <FormWrapper>
            <Form>
              <div
                style={{
                  display: 'flex',
                  gap: '30px',
                  marginBottom: '1.5rem',
                  paddingTop: '1.5rem',
                }}
              >
                <div
                  style={{
                    width: '100%',
                  }}
                >
                  <div>
                    <Trans>Native Token</Trans>
                  </div>
                  <NativeParent>
                    <FaucetInput disabled value="Ethereum" />
                    <ButtonSecondary
                      onClick={(e) => {
                        // prevent page reload
                        e.preventDefault()
                        !account
                          ? toggleWalletModal()
                          : mintMutation.mutate({
                              address: account,
                            })
                      }}
                    >
                      Mint Test Tokens
                    </ButtonSecondary>
                  </NativeParent>
                </div>
              </div>
            </Form>

            <Form>
              <FaucetParent>
                <FaucetContainer>
                  <Trans>Select Token</Trans>
                  {selectedTokenAddress && (
                    <FaucetDropDown
                      currentToken={selectedToken}
                      updateCurrentToken={setSelectedToken}
                      currentTokenAddress={selectedTokenAddress}
                      updateSelectedTokenAddress={setSelectedTokenAddress}
                      availableTokens={faucetTokens}
                    />
                  )}
                  <Trans>Token Contract Address</Trans>
                  {selectedTokenAddress && <FaucetTokenAddressPanel tokenAddress={selectedTokenAddress} />}
                </FaucetContainer>
                <div className="button-div">
                  <ButtonSecondary
                    disabled={!claimable}
                    onClick={(e) => {
                      // prevent page reload
                      e.preventDefault()
                      !isBITFINITYNetwork ? toggleWalletModal() : onClaimToken()
                    }}
                  >
                    Mint Test Tokens
                  </ButtonSecondary>
                </div>
              </FaucetParent>
              <div style={{ display: 'flex', gap: '30px', marginBottom: '40px' }}>
                <div
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    width: '100%',
                    gap: '8px',
                  }}
                >
                  <Trans>How it works:</Trans>
                  <p>
                    You can send a request to the faucet every 60 seconds and, if not already done, import the token
                    into Metamask with the provided token contract address. If the transaction was successfull you will
                    find the claimed tokens in your MetaMask wallet.
                  </p>
                </div>
              </div>
              <div
                style={{
                  display: 'flex',
                  width: '60%',
                  gap: '30px',
                  alignItems: 'center',
                  alignSelf: 'center',
                }}
              >
                <div
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    width: '100%',
                    gap: '8px',
                    alignItems: 'center',
                    alignSelf: 'center',
                  }}
                >
                  <Feedback>{claimFeedback}</Feedback>
                </div>
              </div>
            </Form>
          </FormWrapper>
        </ColumnCenter>
      </Wrapper>
    </>
  )
}
