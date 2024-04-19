import { JsonRpcProvider } from '@ethersproject/providers'
import { parseUnits } from '@ethersproject/units'
import { useWeb3React } from '@web3-react/core'
import toast from 'react-hot-toast'
import { useMutation } from 'react-query'

interface MutationFnProps {
  amount?: string
  address: string
}

const JSON_PRC_URL = 'https://4fe7g-7iaaa-aaaak-aegcq-cai.raw.ic0.app'
const TOKEN_AMOUNT = '0x8ac7230489e80000'

const useMint = () => {
  const { account } = useWeb3React()

  const mutation = useMutation({
    mutationFn: async ({ amount }: MutationFnProps) => {
      const parsedAmount = amount ? parseUnits(amount, 8).toHexString() : TOKEN_AMOUNT
      const provider = new JsonRpcProvider(JSON_PRC_URL)
      await provider.send('ic_mintEVMToken', [account, parsedAmount])
    },
    onSuccess: () => {
      toast.success(`Minted successfully...`)
    },
    onError: () => {
      toast.error('Failed to mint...')
    },
  })

  return mutation
}

export default useMint
