import { BaseConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { Principal } from '@dfinity/principal';

import { BtcBridgeActor } from './ic';
import {
  BFT_BRIDGE_ETH_ADDRESS,
  BTC_BRIDGE_CANISTER_ID,
  BTC_BRIDGE_ETH_ADDRESS,
  BTC_TOKEN_WRAPPED_ADDRESS
} from './constants';
import { encodeBtcAddress, ethAddrToSubaccount } from './utils';
import WrappedTokenABI from './abi/WrappedToken';
import BFTBridgeABI from './abi/BFTBridge';
import { wait } from './tests/utils';
import * as ethers from 'ethers';

type EthAddr = `0x${string}`;

interface BtcBridgeOptions {
  btc: BtcConnector;
  ethWallet: ethers.BaseWallet;
  btcAddress?: EthAddr;
  bftAddress?: EthAddr;
  wrappedTokenAddress?: EthAddr;
}

export class BtcBridge {
  protected btc: BtcConnector;
  protected ethWallet: ethers.BaseWallet;
  protected btcAddress?: string;
  protected bftAddress?: string;
  public wrappedTokenAddress: string;

  constructor({
    btc,
    ethWallet,
    bftAddress,
    btcAddress,
    wrappedTokenAddress
  }: BtcBridgeOptions) {
    this.btc = btc;
    this.ethWallet = ethWallet;
    this.btcAddress = btcAddress || BTC_BRIDGE_ETH_ADDRESS!;
    this.bftAddress = bftAddress || BFT_BRIDGE_ETH_ADDRESS!;
    this.wrappedTokenAddress =
      wrappedTokenAddress || BTC_TOKEN_WRAPPED_ADDRESS!;
  }

  async getAddress() {
    const ethAddress = await this.ethWallet.getAddress();

    return ethAddress;
  }

  getWrappedTokenContract() {
    return new ethers.Contract(
      this.wrappedTokenAddress,
      WrappedTokenABI,
      this.ethWallet
    );
  }

  getBftBridgeContract() {
    return new ethers.Contract(this.bftAddress!, BFTBridgeABI, this.ethWallet);
  }

  async getWrappedTokenBalance() {
    const wrappedTokenContract = this.getWrappedTokenContract();

    const ethAddress = await this.getAddress();

    return await wrappedTokenContract.balanceOf(ethAddress);
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

  async getBTCAddress() {
    const ethAddress = await this.getAddress();
    const btcAddress = await BtcBridgeActor.get_btc_address({
      owner: [Principal.fromText(BTC_BRIDGE_CANISTER_ID)],
      subaccount: [ethAddrToSubaccount(ethAddress)]
    });
    return btcAddress;
  }

  async bridgeBtcToEvm() {
    const ethAddress = await this.getAddress();
    return await BtcBridgeActor.btc_to_erc20(ethAddress);
  }

  async bridgeEVMc(address: string, satoshis: number) {
    const wrappedTokenContract = this.getWrappedTokenContract();

    let tx = await wrappedTokenContract.approve(this.bftAddress, satoshis);
    await tx.wait(2);

    await wait(10000);

    const bftBridgeContract = this.getBftBridgeContract();

    tx = await bftBridgeContract.burn(
      satoshis,
      this.wrappedTokenAddress,
      `0x${encodeBtcAddress(address)}`
    );
    await tx.wait(2);
  }
}
