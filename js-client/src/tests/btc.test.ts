import 'dotenv/config';

import { describe, expect, test } from 'vitest';
import bitcore from 'bitcore-lib';

import { BtcBridge } from '../btc';
import { randomWallet, wait, mintNativeToken, execBitcoinCmd } from './utils';
import { EthAddress } from '../validation';

describe.sequential(
  'btc',
  () => {
    const wallet = randomWallet();

    const btcBridge = new BtcBridge({ provider: wallet });

    test('get balance', async () => {
      await mintNativeToken(wallet.address, '10000000000000000');

      const wrappedBalance = await btcBridge.getWrappedTokenBalance(
        wallet.address as EthAddress
      );

      expect(wrappedBalance).toStrictEqual(0n);
    });

    test('bridge to evm', async () => {
      const btcAddress = await btcBridge.getBTCAddress(
        wallet.address as EthAddress
      );

      await execBitcoinCmd(
        `sendtoaddress "${btcAddress}" ${bitcore.Unit.fromSatoshis(1000000000).toBTC()}`
      );
      await execBitcoinCmd(
        `generatetoaddress 1 "bcrt1quv0zt5ep4ksx8l2tgtgpfd7fsz6grr0wek3rg7"`
      );
      await wait(10000);

      await btcBridge.bridgeBtc(wallet.address as EthAddress);

      await wait(5000);

      const wrappedBalance = await btcBridge.getWrappedTokenBalance(
        wallet.address as EthAddress
      );

      expect(wrappedBalance).toStrictEqual(999998990n);
    });

    test('bridge from evm', async () => {
      const address = (await execBitcoinCmd('getnewaddress')).trim();

      await btcBridge.bridgeEVMc(address, 100000000);

      await execBitcoinCmd(
        `generatetoaddress 1 "bcrt1quv0zt5ep4ksx8l2tgtgpfd7fsz6grr0wek3rg7"`
      );

      await wait(5000);

      const wrappedBalance = await btcBridge.getWrappedTokenBalance(
        wallet.address as EthAddress
      );
      expect(wrappedBalance).toStrictEqual(899998990n);
    });
  },
  180000
);
