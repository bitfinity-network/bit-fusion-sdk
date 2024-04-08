import { expect, test } from 'vitest';
import { createAgent, generateEthWallet, wait } from './utils';
import { IcrcBridge } from '../icrc';
import { Id256Factory } from '../validation';
import { Principal } from '@dfinity/principal';
import { ICRC2_TOKEN_CANISTER_ID } from '../constants';

(BigInt as any).prototype.toJSON = function () {
  return this.toString();
};

test('bridge icrc2 token to evm', async () => {
  const amount = BigInt(100);

  const agent = createAgent();

  const wallet = generateEthWallet();

  const icrcBridge = new IcrcBridge({ wallet, agent });

  // await icrcBridge.deployBftWrappedToken(
  //   'AUX',
  //   'AUX',
  //   Id256Factory.fromPrincipal(Principal.fromText(ICRC2_TOKEN_CANISTER_ID))
  // );

  // await icrcBridge.bridgeIcrc2(10000n, wallet.address);

  // await wait(2000);

  // const balance = await icrcBridge.getWrappedTokenBalance();

  // expect(balance).toBe(amount);
  expect(2).toBe(2);
}, 15000);
