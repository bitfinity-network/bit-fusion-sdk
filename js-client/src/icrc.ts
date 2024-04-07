import { BaseConnector as BtcConnector } from '@particle-network/btc-connectkit';
import { WalletClient as EthConnector, getContract } from 'viem';
import { Principal } from '@dfinity/principal';

// import { createBtcBridgeActor, BtcBridgeActor } from './ic';
import {
  createICRC1Actor,
  createICRC2MinterActor,
  ICRC1,
  ICRC2Minter
} from './ic';
import { ICRC2_MINTER_CANISTER_ID, ICRC2_TOKEN_CANISTER_ID } from './constants';
import { generateOperationId, wait } from './tests/utils';
// import { expect } from 'vitest';
import { Address, Id256Factory } from './validation';
import BftBridgeABI from './abi/BFTBridge';
import WrappedTokenABI from './abi/WrappedToken';

export class IcrcBridge {
  // protected btcBridgeActor: typeof BtcBridgeActor;
  // protected ICRC1Actor: typeof ICRC1;
  icrc2Minter: typeof ICRC2Minter;
  tokenId: Principal

  constructor(
    protected id: string,
    protected token: typeof ICRC1,
    protected eth: EthConnector
  ) {
    this.icrc2Minter = createICRC2MinterActor(ICRC2_MINTER_CANISTER_ID);
    this.tokenId = Principal.fromText(id)
  }

  async getBftBridgeContract() {
    const [bridgeAddress] = await this.icrc2Minter.get_bft_bridge_contract();

    if (!bridgeAddress) {
      // expect.fail('Failed to get bft bridge contract address');
    }

    return getContract({
      address: bridgeAddress as `0x${string}`,
      abi: BftBridgeABI,
      client: { public: this.eth, wallet: this.eth }
    });
  }

  async getWrappedTokenBalance() {
    const wrappedTokenContract = await this.getWrappedTokenContract();

    const [ethAddress] = await this.eth.getAddresses();

    return await wrappedTokenContract.read.balanceOf([ethAddress]);
  }

  async getWrappedTokenContract() {
    const bftBridgeContract = await this.getBftBridgeContract()

    const wrappedTokenAddress = await bftBridgeContract.read.getWrappedToken(
      [Id256Factory.fromPrincipal(this.tokenId)]
    );

    return getContract({
      address: wrappedTokenAddress as `0x${string}`,
      abi: WrappedTokenABI,
      client: { public: this.eth, wallet: this.eth }
    });
  }

  async bridgeIcrc(amount: bigint) {
    const tokenFee = await this.token.icrc1_fee();

    const response = await this.token.icrc2_approve({
      fee: [tokenFee],
      memo: [],
      from_subaccount: [],
      created_at_time: [],
      amount: amount,
      expected_allowance: [],
      expires_at: [],
      spender: {
        owner: Principal.fromText(ICRC2_MINTER_CANISTER_ID),
        subaccount: []
      }
    });

    if ('Err' in response) {
      // expect.fail(
      //   `Failed to approve the ${amount} ICRC2 token: ${JSON.stringify(response.Err)}`
      // );
    }

    const [ethAddress] = await this.eth.getAddresses();

    const burnIcrc2Response = await this.icrc2Minter.burn_icrc2({
      operation_id: generateOperationId(),
      from_subaccount: [],
      icrc2_token_principal: this.tokenId,
      recipient_address: ethAddress,
      amount: amount.toString()
    });

    if ('Err' in burnIcrc2Response) {
      // expect.fail(
      //   `ICRC2 Minter failed to burn the ${amount} ICRC2 tokens: ${JSON.stringify(burnIcrc2Response.Err)}`
      // );
    }



    // const [bridgeAddress] = await this.icrc2Minter.get_bft_bridge_contract();
    //
    // if (!bridgeAddress) {
    //   expect.fail('Failed to get bft bridge contract address');
    // }

    // const bftBridgeContract = getContract(bridgeAddress!, BftBridgeABI.abi);

    // const bftBridgeContract = await this.getBftBridgeContract()
    //
    // const wrappedToken = await bftBridgeContract.read.getWrappedToken(
    //   [Id256Factory.fromPrincipal(this.tokenId)]
    // );
    // console.log('wrappedToken:', wrappedToken);
    //
    // const wrappedTokenAddress = new Address(wrappedToken as string);
    // if (wrappedTokenAddress.isZero()) {
    //   expect.fail('Invalid wrapped token address');
    // }
    //
    // const wrappedTokenContract = getContract(
    //   wrappedTokenAddress.getAddress(),
    //   WrappedTokenABI.abi
    // );
    //
    // await wait(2000);
    //
    // console.log('wallet address:', wallet.address);
    // const balance = await wrappedTokenContract.balanceOf(wallet.address);
    //
    // expect(balance).toBe(amount);
  }
}
