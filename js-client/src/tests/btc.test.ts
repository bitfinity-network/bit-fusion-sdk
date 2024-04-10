import { test } from 'vitest';
import { createActor as createBtcBridgeActor } from '../canisters/btc-bridge';
import { createICRC1Actor } from '../ic';

import { BTC_BRIDGE_CANISTER_ID } from '../constants';
import {
  createAgent,
  generateBitcoinToAddress,
  generateWallet,
  wait
} from './utils';
import { Principal } from '@dfinity/principal';
import { ethAddrToSubaccount } from '../utils';

test('test btc bridge', async () => {
  const agent = createAgent();

  const wallet = generateWallet();

  const btcBridge = createBtcBridgeActor(BTC_BRIDGE_CANISTER_ID, { agent });

  const btcAddress = await btcBridge.get_btc_address({
    owner: [Principal.fromText(BTC_BRIDGE_CANISTER_ID)],
    subaccount: [ethAddrToSubaccount(wallet.address)]
  });

  generateBitcoinToAddress(btcAddress);

  await wait(10000);

  await btcBridge.btc_to_erc20(wallet.address);

  await wait(5000);

  
});
