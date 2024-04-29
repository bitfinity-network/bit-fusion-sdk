import 'dotenv/config';
import { describe, expect, test, vi } from 'vitest';
import { OKXConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { createWalletClient, http, defineChain } from 'viem';
import { privateKeyToAccount, generatePrivateKey } from 'viem/accounts';
import { exec } from 'child_process';
import bitcore from 'bitcore-lib';

import { BtcBridge } from '../btc';
import { mintNativeToken, wait } from './utils';

export const execCmd = (cmd: string): Promise<string> => {
  return new Promise((resolve, reject) => {
    exec(cmd, (err, stdout) => {
      if (err) {
        return reject(err);
      } else if (stdout) {
        resolve(stdout);
      }
    });
  });
};

export const execBitcoinCmd = (cmd: string) => {
  return execCmd(`${process.env.BITCOIN_CMD} ${cmd}`);
};

const account = privateKeyToAccount(generatePrivateKey());

export const evmc = defineChain({
  id: 355113,
  name: 'EVMc',
  testnet: true,
  nativeCurrency: {
    decimals: 18,
    name: 'BTF',
    symbol: 'BTF'
  },
  rpcUrls: {
    default: {
      http: [process.env.RPC_URL!]
    }
  }
});

describe.sequential(
  'btc',
  () => {
    const eth = createWalletClient({
      account,
      chain: evmc,
      transport: http()
    });

    const btc = new BtcConnector();

    vi.spyOn(btc, 'sendBitcoin').mockImplementation(async (address, amount) => {
      const result = await execBitcoinCmd(
        `sendtoaddress "${address}" ${bitcore.Unit.fromSatoshis(amount).toBTC()}`
      );
      await execBitcoinCmd(
        `generatetoaddress 1 "bcrt1quv0zt5ep4ksx8l2tgtgpfd7fsz6grr0wek3rg7"`
      );
      await wait(10000);
      return result;
    });

    const btcBridge = new BtcBridge(btc, eth);

    test('get balance', async () => {
      const ethAddress = await btcBridge.getAddress();
      expect(ethAddress).toStrictEqual(account.address);

      await mintNativeToken(ethAddress, '10000000000000000');

      const wrappedBalance = await btcBridge.getWrappedTokenBalance();

      expect(wrappedBalance).toStrictEqual(0n);
    });

    test('bridge to evm', async () => {
      await btcBridge.bridgeBtc(1000000000);

      await wait(5000);

      const wrappedBalance = await btcBridge.getWrappedTokenBalance();

      expect(wrappedBalance).toStrictEqual(999998990n);
    });

    test('bridge from evm', async () => {
      const address = (await execBitcoinCmd('getnewaddress')).trim();

      await btcBridge.bridgeEVMc(address, 100000000);

      await execBitcoinCmd(
        `generatetoaddress 1 "bcrt1quv0zt5ep4ksx8l2tgtgpfd7fsz6grr0wek3rg7"`
      );

      await wait(5000);

      const wrappedBalance = await btcBridge.getWrappedTokenBalance();
      expect(wrappedBalance).toStrictEqual(899998990n);
    });
  },
  180000
);
