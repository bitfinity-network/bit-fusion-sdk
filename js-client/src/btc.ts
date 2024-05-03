import { Principal } from '@dfinity/principal';
import * as ethers from 'ethers';

import { BtcBridgeActor } from './ic';
import {
  BFT_ETH_ADDRESS,
  BTC_BRIDGE_CANISTER_ID,
  BTC_TOKEN_WRAPPED_ADDRESS
} from './constants';
import { encodeBtcAddress, ethAddrToSubaccount } from './utils';
import WrappedTokenABI from './abi/WrappedToken';
import BFTBridgeABI from './abi/BFTBridge';
import { wait } from './tests/utils';

import { EthAddress } from './validation';

interface BtcBridgeOptions {
  provider: ethers.Signer;
  bftAddress?: EthAddress;
  wrappedTokenAddress?: EthAddress;
}

export class BtcBridge {
  protected provider: ethers.Signer;
  protected bftAddress: EthAddress;
  public wrappedTokenAddress: string;

  constructor({ provider, bftAddress, wrappedTokenAddress }: BtcBridgeOptions) {
    this.provider = provider;
    this.bftAddress = bftAddress || (BFT_ETH_ADDRESS! as EthAddress);
    this.wrappedTokenAddress =
      wrappedTokenAddress || BTC_TOKEN_WRAPPED_ADDRESS!;
  }

  getWrappedTokenContract() {
    return new ethers.Contract(
      this.wrappedTokenAddress,
      WrappedTokenABI,
      this.provider
    );
  }

  getBftBridgeContract() {
    return new ethers.Contract(this.bftAddress!, BFTBridgeABI, this.provider);
  }

  async getWrappedTokenBalance(address: EthAddress) {
    const wrappedTokenContract = this.getWrappedTokenContract();

    return await wrappedTokenContract.balanceOf(address);
  }

  async bridgeBtc(address: EthAddress) {
    return await BtcBridgeActor.btc_to_erc20(address);
  }

  async getBTCAddress(address: EthAddress) {
    const btcAddress = await BtcBridgeActor.get_btc_address({
      owner: [Principal.fromText(BTC_BRIDGE_CANISTER_ID)],
      subaccount: [ethAddrToSubaccount(address)]
    });

    return btcAddress;
  }

  async bridgeEVMc(address: string, satoshis: number) {
    const wrappedTokenContract = this.getWrappedTokenContract();

    const approveTx = await wrappedTokenContract.approve(
      this.bftAddress,
      satoshis
    );
    await approveTx.wait(2);

    await wait(10000);

    const bftBridgeContract = this.getBftBridgeContract();

    const burnTx = await bftBridgeContract.burn(
      satoshis,
      this.wrappedTokenAddress,
      `0x${encodeBtcAddress(address)}`
    );

    await burnTx.wait(2);
  }
}
