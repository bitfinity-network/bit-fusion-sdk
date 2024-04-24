import 'dotenv/config';
import { describe, expect, test, vi } from 'vitest';
import { OKXConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { exec } from 'child_process';
import bitcore from 'bitcore-lib';

import { BtcBridge } from '../btc';
import { generateWallet, wait } from './utils';

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

export async function mintNativeToken(toAddress: string, amount: string) {
  const response = await fetch(process.env.ETH_RPC_URL!, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: '67',
      method: 'ic_mintNativeToken',
      params: [toAddress, amount]
    })
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
}

describe.sequential(
  'btc',
  () => {
    const wallet = generateWallet();

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

    const btcBridge = new BtcBridge({ btc, ethWallet: wallet });

    test('get balance', async () => {
      const ethAddress = await btcBridge.getAddress();
      expect(ethAddress).toStrictEqual(wallet.address);

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
