import { Actor } from '@dfinity/agent';
import * as ethers from 'ethers';

import { RuneActor } from './ic';
import { EthAddress, Id256Factory } from './validation';
import WrappedTokenABI from './abi/WrappedToken';
import BftBridgeABI from './abi/BFTBridge';
import { wait } from './tests/utils';
import { encodeBtcAddress } from './utils';
import { BFT_ETH_ADDRESS } from './constants';

interface RuneBridgeOptions {
  bftAddress?: EthAddress;
  provider: ethers.Signer;
}

export class RuneBridge {
  protected bftAddress: EthAddress;
  protected provider: ethers.Signer;

  constructor({ provider, bftAddress }: RuneBridgeOptions) {
    this.bftAddress = bftAddress || (BFT_ETH_ADDRESS! as EthAddress);
    this.provider = provider;
  }

  /**
   *
   * dfx canister call rune-bridge get_deposit_address "(\"$ETH_WALLET_ADDRESS\")"
   *
   */
  async getDepositAddress(ethAddress: EthAddress) {
    const result = await RuneActor.get_deposit_address(ethAddress);

    if (!('Ok' in result)) {
      throw new Error('Err');
    }

    return result.Ok;
  }

  private getBftBridgeContract() {
    return new ethers.Contract(this.bftAddress, BftBridgeABI, this.provider);
  }

  private async getWrappedTokenContract() {
    const address = await this.getWrappedTokenEthAddress();

    return new ethers.Contract(address, WrappedTokenABI, this.provider);
  }

  /**
   *
   * TOKEN_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- create-token \
   *   --bft-bridge-address="$BFT_ETH_ADDRESS" \
   *   --token-name=RUNE \
   *   --token-id="$RUNE_BRIDGE" \
   *   --evm-canister="$EVM" \
   *   --wallet="$ETH_WALLET")
   *
   */
  async getWrappedTokenEthAddress(): Promise<string> {
    const contract = this.getBftBridgeContract();

    // TODO: is the TOKEN_ETH_ADDRESS only depends on token-id?
    return await contract.getWrappedToken(
      Id256Factory.fromPrincipal(Actor.canisterIdOf(RuneActor))
    );
  }

  async getWrappedTokenBalance(address: EthAddress) {
    const wrappedTokenContract = await this.getWrappedTokenContract();

    return await wrappedTokenContract.balanceOf(address);
  }

  async bridgeBtc(ethAddress: EthAddress) {
    for (let attempt = 0; attempt < 3; attempt++) {
      const result = await RuneActor.deposit(ethAddress);

      if ('Ok' in result) {
        return result.Ok;
      }

      await wait(7000);
    }
  }

  /**
   *
   * cargo run -q -p create_bft_bridge_tool -- burn-wrapped \
   *   --wallet="$ETH_WALLET" \
   *   --evm-canister="$EVM" \
   *   --bft-bridge="$BFT_ETH_ADDRESS" \
   *   --token-address="$TOKEN_ETH_ADDRESS" \
   *   --address="$RECEIVER" \
   *   --amount=10
   *
   */
  async bridgeEVMc(address: string, satoshis: number) {
    const wrappedTokenContract = await this.getWrappedTokenContract();

    await wrappedTokenContract.approve(this.bftAddress, satoshis);

    await wait(15000);

    const bftBridgeContract = this.getBftBridgeContract();

    const tokenAddress = await this.getWrappedTokenEthAddress();

    await bftBridgeContract.burn(
      satoshis,
      tokenAddress,
      `0x${encodeBtcAddress(address)}`
    );
  }

  async getRunesBalance(address: string) {
    return await RuneActor.get_rune_balances(address);
  }
}
