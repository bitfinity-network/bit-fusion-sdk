import 'dotenv/config';
import { describe, expect, test, vi } from 'vitest';
import { BtcBridge } from '../btc';
import { OKXConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { createWalletClient, http, defineChain } from 'viem';
import { privateKeyToAccount, generatePrivateKey } from 'viem/accounts';
import { exec } from 'child_process';
import { wait } from './utils';

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

export const execBitcionCmd = (cmd: string) => {
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
      http: [process.env.ETH_RPC_URL!]
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

    vi.spyOn(btc, 'sendBitcoin').mockImplementation(async (address) => {
      const result = await execBitcionCmd(`sendtoaddress "${address}" 1`);
      await execBitcionCmd(
        `generatetoaddress 1 "bcrt1quv0zt5ep4ksx8l2tgtgpfd7fsz6grr0wek3rg7"`
      );
      await wait(10000);
      return result;
    });

    test('get balance', async () => {
      const btcBridge = new BtcBridge(btc, eth);
      const wrappedBalance = await btcBridge.getWrappedTokenBalance();

      expect(wrappedBalance).toStrictEqual(0n);
    });

    test('bridge to evm', async () => {
      const btcBridge = new BtcBridge(btc, eth);

      const ethAddress = await btcBridge.getAddress();
      expect(ethAddress).toStrictEqual(account.address);

      await btcBridge.bridgeBtc(1000);

      await wait(10000);

      const wrappedBalance = await btcBridge.getWrappedTokenBalance();

      expect(wrappedBalance).toStrictEqual(99998990n);
    });
  },
  60000
);
