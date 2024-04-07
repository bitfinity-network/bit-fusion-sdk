import { BaseConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { WalletClient as EthConnector } from 'viem';
import { Principal } from '@dfinity/principal';

import { createBtcBridgeActor, BtcBridgeActor } from './ic';
import { BTC_BRIDGE_CANISTER_ID } from './constants';
import { ethAddrToSubaccount } from './utils';

export abstract class Bridge {
  constructor(
    protected btc: BtcConnector,
    protected eth: EthConnector
  ) {}
}

export class BtcBridge extends Bridge {
  protected btcBridgeActor: typeof BtcBridgeActor;

  constructor(
    protected btc: BtcConnector,
    protected eth: EthConnector
  ) {
    super(btc, eth);

    this.btcBridgeActor = createBtcBridgeActor(BTC_BRIDGE_CANISTER_ID);
  }

  async bridge(satoshis: number) {
    const [ethAddress] = await this.eth.getAddresses();

    const btcAddress = await this.btcBridgeActor.get_btc_address({
      owner: [Principal.fromText(BTC_BRIDGE_CANISTER_ID)],
      subaccount: [ethAddrToSubaccount(ethAddress)]
    });

    await this.btc.sendBitcoin(btcAddress, satoshis);

    await this.btcBridgeActor.btc_to_erc20(ethAddress);
  }
}
