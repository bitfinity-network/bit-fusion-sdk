import { Currency } from 'sdk-core/src/index'

export function currencyId(currency?: Currency): string {
  if (currency?.isNative) return 'ETH'
  if (currency?.isToken) return currency.address
  throw new Error('invalid currency')
}
