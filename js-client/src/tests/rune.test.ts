import 'dotenv/config';
import { describe, expect, test, vi } from 'vitest';
import { OKXConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { createWalletClient, http, defineChain } from 'viem';
import { privateKeyToAccount, generatePrivateKey } from 'viem/accounts';
import { exec } from 'child_process';
import bitcore from 'bitcore-lib';

import { RuneBridge } from '../rune';
import { wait, execOrdCmd, execBitcoinCmd, generateWallet } from './utils';

describe.sequential(
  'rune',
  () => {
    const RUNE_NAME = 'SUPERMAXRUNENAME';

    const wallet = generateWallet();

    const btcBridge = new RuneBridge(wallet);

    test('canister id', () => {
      expect(btcBridge.canisterId()).toStrictEqual(
        process.env.RUNE_BRIDGE_CANISTER_ID
      );
    });

    // test('get deposit address', async () => {
    //   console.log();
    // });

    test('to evm', async () => {
      const toAddress = '0x650063413de76656636ea6bebd59c71d74c0dc41';

      const address = await btcBridge.getDepositAddress(toAddress);

      console.log(await btcBridge.getTokenEthAddress());

      // mint
      // ./bitcoin/bin/ord -r --bitcoin-rpc-username ic-btc-integration --bitcoin-rpc-password QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E= --index-runes --data-dir target/bc wallet --server-url http://0.0.0.0:8000  mint --fee-rate 10 --rune SUPERMAXRUNENAME

      // console.log(
      //   await execOrdCmd(
      //     `wallet --server-url http://0.0.0.0:8000 send --fee-rate 10 ${address} 10:${RUNE_NAME}`
      //   )
      // );
      //
      // await execBitcoinCmd(`sendtoaddress ${address} 0.0049`);
      // await execBitcoinCmd(
      //   `generatetoaddress 1 bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts`
      // );
      //
      // await wait(1000);
      //
      // for (let i = 0; i < 2; i++) {
      //   await btcBridge.deposit(toAddress);
      // }


    });
  },
  180000
);
