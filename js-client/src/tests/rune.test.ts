import 'dotenv/config';
import { describe, expect, test } from 'vitest';

import { RuneBridge } from '../rune';
import {
  execBitcoinCmd,
  execOrdReceive,
  execOrdSend,
  mintNativeToken,
  randomWallet,
  wait
} from './utils';

describe.sequential(
  'rune',
  async () => {
    const RUNE_NAME = 'SUPERMAXRUNENAME';

    const wallet = randomWallet();

    await mintNativeToken(wallet.address, '10000000000000000');

    const runeBridge = new RuneBridge({ provider: wallet });

    test('bridge to evm', async () => {
      const toAddress = wallet.address as `0x${string}`;

      console.log(toAddress);

      const address = await runeBridge.getDepositAddress(toAddress);

      const wrappedBalance = await runeBridge.getWrappedTokenBalance(toAddress);
      expect(wrappedBalance).toStrictEqual(0n);

      const sendResult = await execOrdSend(address, RUNE_NAME);
      expect(
        sendResult,
        'Impossible to send rune. Is it mined to the wallet?'
      ).not.toStrictEqual(null);

      await execBitcoinCmd(`sendtoaddress ${address} 0.0049`);
      await execBitcoinCmd(
        `generatetoaddress 1 bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts`
      );

      await runeBridge.bridgeBtc(toAddress);

      await wait(15000);

      const wrappedBalance2 =
        await runeBridge.getWrappedTokenBalance(toAddress);

      expect(wrappedBalance2).toStrictEqual(1000n);
    });

    test('bridge from evm', async () => {
      const toAddress = await execOrdReceive();

      await runeBridge.bridgeEVMc(toAddress, 100);

      await wait(15000);

      const wrappedBalance = await runeBridge.getWrappedTokenBalance(
        wallet.address as `0x${string}`
      );
      expect(wrappedBalance).toStrictEqual(900n);
    });
  },
  180000
);
