import { test } from 'vitest';
import { BtcBridge } from '../btc';


test('test btc bridge', async () => {
  // all wallets api must be mocked!
  const btcBridge = new BtcBridge(undefined as any, undefined as any);

  await btcBridge.bridgeBtc(1000)

  // implement some checks
});
