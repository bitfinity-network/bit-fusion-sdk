import { expect, test } from 'vitest';
import {
  createAgent,
  generateWallet,
  identityFromSeed,
  wait
} from './utils';
import { IcrcBridge } from '../icrc';
import { LOCAL_TEST_SEED_PHRASE } from '../constants';
import { Id256Factory } from '../validation';

(BigInt as any).prototype.toJSON = function () {
  return this.toString();
};

test('bridge icrc2 token to evm', async () => {
  const wallet = generateWallet();

  const agent = createAgent();

  const icrcBricdge = await IcrcBridge.create({
    wallet,
    agent
  });

  await icrcBricdge.deployBftWrappedToken('AUX', 'AUX');

  await wait(5000);

  const wrappedToken = await icrcBricdge.getWrappedTokenContract();

  let balance = await wrappedToken.balanceOf(wallet.address);

  console.log('ibalance', balance);

  const amount = 100000n;

  console.log('wallet.address', wallet.address);

  await icrcBricdge.bridgeIcrc2(amount, wallet.address);

  await wait(10000);

  // const wrappedToken = await icrcBricdge.getWrappedTokenContract();

  balance = await wrappedToken.balanceOf(wallet.address);

  console.log('out', { balance, amount });

  expect(balance).toBeGreaterThan(0n);
}, 30000);

test('bridge evmc tokens to icrc2', async () => {
  const amount = 1000n;

  const wallet = generateWallet();

  const agent = createAgent();

  const icrcBricdge = await IcrcBridge.create({
    wallet,
    agent
  });

  const [bftBridgeAddress] =
    await icrcBricdge.icrc2Minter.get_bft_bridge_contract();

  const wrappedToken = await icrcBricdge.getWrappedTokenContract();

  const identity = identityFromSeed(LOCAL_TEST_SEED_PHRASE);
  const userPrincipal = (await identity).getPrincipal();

  const initialBalance = await icrcBricdge.baseToken.icrc1_balance_of({
    owner: userPrincipal,
    subaccount: []
  });

  await wrappedToken.approve(bftBridgeAddress, amount);

  const wrappedTokenAddress = await wrappedToken.getAddress();

  const tx = await icrcBricdge.bftBridge.burn(
    amount,
    wrappedTokenAddress,
    Id256Factory.fromPrincipal(userPrincipal)
  );
  await tx.wait(2);

  const finalBalance = await icrcBricdge.baseToken.icrc1_balance_of({
    owner: (await identity).getPrincipal(),
    subaccount: []
  });

  expect(finalBalance).toBeGreaterThan(initialBalance);
}, 15000);
