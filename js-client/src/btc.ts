import { BaseConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { WalletClient as EthConnector } from 'viem';
import { Principal } from '@dfinity/principal';

import { BtcBridgeActor } from './ic';
import { BTC_BRIDGE_CANISTER_ID } from './constants';
import { ethAddrToSubaccount } from './utils';

export class BtcBridge {
  constructor(
    protected btc: BtcConnector,
    protected eth: EthConnector
  ) {}

  async bridgeBtc(satoshis: number) {
    const [ethAddress] = await this.eth.getAddresses();

    const btcAddress = await BtcBridgeActor.get_btc_address({
      owner: [Principal.fromText(BTC_BRIDGE_CANISTER_ID)],
      subaccount: [ethAddrToSubaccount(ethAddress)]
    });

    await this.btc.sendBitcoin(btcAddress, satoshis);

    await BtcBridgeActor.btc_to_erc20(ethAddress);
  }
}
