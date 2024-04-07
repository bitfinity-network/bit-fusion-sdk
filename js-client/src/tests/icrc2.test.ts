import { expect, test } from 'vitest';
import { Principal } from '@dfinity/principal';
import { createICRC1Actor } from '../ic';
import { createAgent, wait } from './utils';
import canisters from '../../../.dfx/local/canister_ids.json';
import { IcrcBridge } from '../icrc';

BigInt.prototype.toJSON = function () {
  return this.toString();
};

const ICRC2_TOKEN_CANISTER_ID = Principal.fromText(canisters.token2.local);

test('bridge icrc2 token to evm', async () => {
  const amount = BigInt(100);

  const agent = createAgent();

  const token = createICRC1Actor(ICRC2_TOKEN_CANISTER_ID, { agent });

  const icrcBridge = new IcrcBridge(
    ICRC2_TOKEN_CANISTER_ID.toText(),
    token,
    undefined as any
  );

  await icrcBridge.bridgeIcrc(10000n);

  await wait(2000);

  const balance = await icrcBridge.getWrappedTokenBalance();

  expect(balance).toBe(amount);
}, 15000);
