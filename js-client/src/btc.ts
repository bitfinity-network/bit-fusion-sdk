import { BaseConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { WalletClient as EthConnector, getContract } from 'viem';
import { Principal } from '@dfinity/principal';

import { BtcBridgeActor } from './ic';
import { BTC_BRIDGE_CANISTER_ID } from './constants';
import { ethAddrToSubaccount } from './utils';
import WrappedTokenABI from './abi/WrappedToken';

export class BtcBridge {
  constructor(
    protected btc: BtcConnector,
    protected eth: EthConnector
  ) {}

  async getAddress() {
    const [ethAddress] = await this.eth.getAddresses();

    return ethAddress;
  }

  async getWrappedTokenContract() {
    return getContract({
      address: process.env.BITCOIN_TOKEN_WRAPPED_ADDRESS as `0x${string}`,
      abi: WrappedTokenABI,
      client: this.eth
    });
  }

  async getWrappedTokenBalance() {
    const wrappedTokenContract = await this.getWrappedTokenContract();

    const ethAddress = await this.getAddress();

    return await wrappedTokenContract.read.balanceOf([ethAddress]);
  }

  async bridgeBtc(satoshis: number) {
    const ethAddress = await this.getAddress();

    const btcAddress = await BtcBridgeActor.get_btc_address({
      owner: [Principal.fromText(BTC_BRIDGE_CANISTER_ID)],
      subaccount: [ethAddrToSubaccount(ethAddress)]
    });

    await this.btc.sendBitcoin(btcAddress, satoshis);

    return await BtcBridgeActor.btc_to_erc20(ethAddress);
  }
}
