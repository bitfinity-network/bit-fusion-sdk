import { createBtcBridgeActor, RuneActor } from './ic';
import { Actor } from '@dfinity/agent';
import * as ethers from 'ethers';
import { Address, Id256Factory } from './validation';
import BftBridgeABI from './abi/BFTBridge';
import { Buffer } from 'buffer';
// import { BTC_BRIDGE_CANISTER_ID } from './constants';
// import { encodeBtcAddress, ethAddrToSubaccount } from './utils';
// import WrappedTokenABI from './abi/WrappedToken';
// import BFTBridgeABI from './abi/BFTBridge';
// import { wait } from './tests/utils';

type EthAddr = `0x${string}`;

export class RuneBridge {
  protected BTC_ETH_ADDRESS = process.env.BTC_BRIDGE_ETH_ADDRESS as EthAddr;
  protected BFT_ETH_ADDRESS = process.env.BFT_BRIDGE_ETH_ADDRESS as EthAddr;
  protected TOKEN_WRAPPED_ADDRESS = process.env
    .BITCOIN_TOKEN_WRAPPED_ADDRESS as EthAddr;

  constructor(protected provider: ethers.Signer) {}

  canisterId() {
    return Actor.canisterIdOf(RuneActor).toText();
  }
  // async getAddress() {
  //   const [ethAddress] = await this.eth.getAddresses();
  //
  //   return ethAddress;
  // }
  //
  // getWrappedTokenContract() {
  //   return getContract({
  //     address: this.TOKEN_WRAPPED_ADDRESS,
  //     abi: WrappedTokenABI,
  //     client: this.eth
  //   });
  // }
  //
  // getBftBridgeContract() {
  //   return getContract({
  //     address: this.BFT_ETH_ADDRESS,
  //     abi: BFTBridgeABI,
  //     client: this.eth
  //   });
  // }

  async ethAddress() {
    const [address] = await RuneActor.get_evm_address();

    return address;
  }

  async getDepositAddress(ethAddress: EthAddr) {
    // dfx canister call rune-bridge get_deposit_address "(\"$ETH_WALLET_ADDRESS\")"
    const result = await RuneActor.get_deposit_address(ethAddress);

    if (!('Ok' in result)) {
      throw new Error('Err');
    }

    return result.Ok;
  }

  private getBftBridgeContract() {

    return new ethers.Contract(
      process.env.BFT_BRIDGE_ETH_ADDRESS,
      BftBridgeABI,
      this.provider
    );
  }

  async getTokenEthAddress() {
    const contract = this.getBftBridgeContract()

    return  await contract.getWrappedToken(Id256Factory.fromPrincipal(Actor.canisterIdOf(RuneActor)))
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
  }

  async deposit(ethAddress: EthAddr) {
    const result = await RuneActor.deposit(ethAddress);

    console.log(result);
  }
}
