import { Percent } from 'sdk-core/src/index'

export function normalizePriceImpact(priceImpact: Percent): number {
  return Number(priceImpact.multiply(-1).toFixed(3))
}
