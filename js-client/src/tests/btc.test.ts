import { expect, test } from 'vitest';
import { createActor as createBtcBridgeActor } from '../canisters/btc-bridge';
import BftBridgeABI from '../abi/BFTBridge';
import WrappedTokenABI from '../abi/WrappedToken';

import { BTC_BRIDGE_CANISTER_ID, CKBTC_TOKEN_CANISTER_ID } from '../constants';
import {
  createAgent,
  generateBitcoinToAddress,
  generateWallet,
  wait
} from './utils';
import { Principal } from '@dfinity/principal';
import { ethAddrToSubaccount } from '../utils';
import { ethers } from 'ethers';
import { Address, Id256Factory } from '../validation';

test('test btc bridge', async () => {
  const agent = createAgent();

  const wallet = generateWallet();

  const btcBridge = createBtcBridgeActor(BTC_BRIDGE_CANISTER_ID, { agent });

  btcBridge.get_btc_address;

  const btcAddress = await btcBridge.get_btc_address({
    owner: [Principal.fromText(BTC_BRIDGE_CANISTER_ID)],
    subaccount: [ethAddrToSubaccount(wallet.address)]
  });

  console.log('btcAddress:', btcAddress);
  console.log(
    'ethAddrToSubaccount(wallet.address):',
    ethAddrToSubaccount(wallet.address)
  );

  generateBitcoinToAddress(btcAddress);

  await wait(10000);

  const [bftBridgeAddress] = await btcBridge.get_bft_bridge_contract();

  if (!bftBridgeAddress) {
    throw new Error('bft bridge contract not found');
  }

  const bftBridge = new ethers.Contract(bftBridgeAddress, BftBridgeABI, wallet);

  const wrappedTokenAddress = await bftBridge.getWrappedToken(
    Id256Factory.fromPrincipal(Principal.fromText(CKBTC_TOKEN_CANISTER_ID))
  );

  if (new Address(wrappedTokenAddress).isZero()) {
    throw new Error('wrapped token not deployed');
  }

  const wrappedToken = new ethers.Contract(
    wrappedTokenAddress,
    WrappedTokenABI,
    wallet
  );

  const initialBalance = await wrappedToken.balanceOf(wallet.address);

  const response = await btcBridge.btc_to_erc20(wallet.address);
  console.log('response:', response);

  await wait(5000);

  const finalBalance = await wrappedToken.balanceOf(wallet.address);

  expect(finalBalance).toBeGreaterThan(initialBalance);
}, 20000);
