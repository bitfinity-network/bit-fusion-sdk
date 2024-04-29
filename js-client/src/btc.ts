import { BaseConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { WalletClient as EthConnector, getContract } from 'viem';
import { Principal } from '@dfinity/principal';

import { BtcBridgeActor } from './ic';
import { BTC_BRIDGE_CANISTER_ID } from './constants';
import { encodeBtcAddress, ethAddrToSubaccount } from './utils';
import WrappedTokenABI from './abi/WrappedToken';
import BFTBridgeABI from './abi/BFTBridge';
import { wait } from './tests/utils';

type EthAddr = `0x${string}`;

export class BtcBridge {
  protected BFT_ETH_ADDRESS = process.env.BFT_ETH_ADDRESS as EthAddr;
  protected TOKEN_WRAPPED_ADDRESS = process.env
    .BITCOIN_TOKEN_WRAPPED_ADDRESS as EthAddr;

  constructor(
    protected btc: BtcConnector,
    protected eth: EthConnector
  ) {}

  async getAddress() {
    const [ethAddress] = await this.eth.getAddresses();

    return ethAddress;
  }

  getWrappedTokenContract() {
    return getContract({
      address: this.TOKEN_WRAPPED_ADDRESS,
      abi: WrappedTokenABI,
      client: this.eth
    });
  }

  getBftBridgeContract() {
    return getContract({
      address: this.BFT_ETH_ADDRESS,
      abi: BFTBridgeABI,
      client: this.eth
    });
  }

  async getWrappedTokenBalance() {
    const wrappedTokenContract = this.getWrappedTokenContract();

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

  async bridgeEVMc(address: string, satoshis: number) {
    const wrappedTokenContract = this.getWrappedTokenContract();

    await wrappedTokenContract.write.approve([this.BFT_ETH_ADDRESS, satoshis]);

    await wait(10000);

    const bftBridgeContract = this.getBftBridgeContract();

    await bftBridgeContract.write.burn([
      satoshis,
      this.TOKEN_WRAPPED_ADDRESS,
      `0x${encodeBtcAddress(address)}`
    ]);
  }
}
