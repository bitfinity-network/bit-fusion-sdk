import { Principal } from '@dfinity/principal';
import * as ethers from 'ethers';

import { createICRC1Actor, createICRC2MinterActor, ICRC2Minter } from './ic';
import {
  ICRC2_MINTER_CANISTER_ID,
  ICRC2_TOKEN_CANISTER_ID,
  IS_TEST
} from './constants';
import { createAgent, generateOperationId } from './tests/utils';
import { Address, Id256, Id256Factory } from './validation';
// import BftBridgeABI from './abi/BFTBridge';
// import WrappedTokenABI from './abi/WrappedToken';
import { Icrc1IdlFactory } from './ic';
import canisters from '../../.dfx/local/canister_ids.json';
import { Actor, HttpAgent } from '@dfinity/agent';
import { Icrc2Burn } from './canisters/icrc2-minter/icrc2-minter.did';
// import BftBridgeJ from './abi/BFTBridge.json';

export interface IcrcBridgeOptions {
  wallet: ethers.Signer;
  rpcURL?: string;
  // agent?: HttpAgent;
}

export class IcrcBridge {
  icrc2Minter: typeof ICRC2Minter;
  infinityWallet: any;
  wallet: ethers.Signer;

  constructor({ wallet }: IcrcBridgeOptions) {
    this.wallet = wallet;

    console.log('ICRC2_MINTER_CANISTER_ID:', ICRC2_MINTER_CANISTER_ID);

    this.icrc2Minter = createICRC1Actor(
      Principal.fromText(ICRC2_TOKEN_CANISTER_ID)
    );

    // if (window != undefined) {
    //   this.infinityWallet = (window as any).ic?.infinityWallet;
    // }
  }

  // async getBftBridgeContract() {
  //   const [bridgeAddress] = await this.icrc2Minter.get_bft_bridge_contract();

  //   if (!bridgeAddress) {
  //     throw new Error('bridge address not found');
  //   }

  //   if (new Address(bridgeAddress).isZero()) {
  //     throw new Error('bridge contract not deployed');
  //   }

  //   return new ethers.Contract(
  //     bridgeAddress,
  //     BftBridgeJ.abi,
  //     this.wallet.provider
  //   );
  // }

  // async getWrappedTokenBalance() {
  //   const wrappedTokenContract = await this.getWrappedTokenContract();

  //   const address = await this.wallet.getAddress();

  //   return await wrappedTokenContract.balanceOf(address);
  // }

  // async getWrappedTokenContract() {
  //   const bftBridgeContract = await this.getBftBridgeContract();

  //   const wrappedTokenAddress = await bftBridgeContract.getWrappedToken([
  //     Id256Factory.fromPrincipal(Principal.fromText(ICRC2_TOKEN_CANISTER_ID))
  //   ]);

  //   if (new Address(wrappedTokenAddress).isZero()) {
  //     throw new Error('Invalid Address');
  //   }

  //   return new ethers.Contract(
  //     wrappedTokenAddress,
  //     WrappedTokenABI,
  //     this.wallet.provider
  //   );
  // }

  // async deployBftWrappedToken(name: string, symbol: string, fromToken: Id256) {
  //   try {
  //     const bridge = await this.getBftBridgeContract();

  //     return await bridge.deployERC20(name, symbol, fromToken);
  //   } catch (error: any) {
  //     console.log('deployBftWrappedToken err', error);
  //     throw new Error(error.message);
  //   }
  // }

  // async bridgeIcrc2(amount: bigint, recipient: string) {
  //   const Icrc2Burn: Icrc2Burn = {
  //     operation_id: generateOperationId(),
  //     from_subaccount: [],
  //     icrc2_token_principal: Principal.fromText(ICRC2_TOKEN_CANISTER_ID),
  //     recipient_address: recipient,
  //     amount: amount.toString()
  //   };

  //   // eslint-disable-next-line @typescript-eslint/no-unused-vars
  //   const onApproveTxSucess = async (res: any) => {
  //     console.log('approve icrc2 token res:', res);

  //     if ('Ok' in res) {
  //       const burnResponse = await this.icrc2Minter.burn_icrc2(Icrc2Burn);

  //       if ('Err' in burnResponse) {
  //         throw new Error(
  //           `icrc1 minter failed to burn tokens: ${JSON.stringify(burnResponse.Err)}`
  //         );
  //       }

  //       const wrappedToken = await this.getWrappedTokenContract();

  //       return await wrappedToken.balanceOf(recipient);
  //     }

  //     if (IS_TEST) {
  //       const agent = createAgent();
  //       const tokenCanisterId = Principal.fromText(canisters.token2.local);
  //       const token = createICRC1Actor(tokenCanisterId, {
  //         agent
  //       });

  //       const response = await token.icrc2_approve({
  //         fee: [],
  //         memo: [],
  //         from_subaccount: [],
  //         created_at_time: [],
  //         amount: amount,
  //         expected_allowance: [],
  //         expires_at: [],
  //         spender: {
  //           owner: Actor.canisterIdOf(this.icrc2Minter),
  //           subaccount: []
  //         }
  //       });

  //       if ('Err' in response) {
  //         throw new Error(
  //           `failed to approve tokens: ${JSON.stringify(response.Err)}`
  //         );
  //       }

  //       const burnResponse = await this.icrc2Minter.burn_icrc2(Icrc2Burn);

  //       if ('Err' in burnResponse) {
  //         throw new Error(
  //           `failed to burn tokens: ${JSON.stringify(burnResponse.Err)}`
  //         );
  //       }
  //     } else {
  //       const APPROVE_TX = {
  //         idl: Icrc1IdlFactory,
  //         canisterId: ICRC2_TOKEN_CANISTER_ID,
  //         methodName: 'icrc2_approve',
  //         args: [
  //           {
  //             fee: [],
  //             memo: [],
  //             from_subaccount: [],
  //             created_at_time: [],
  //             amount,
  //             expected_allowance: [],
  //             expires_at: [],
  //             spender: {
  //               owner: this.icrc2Minter,
  //               subaccount: []
  //             }
  //           }
  //         ],
  //         onSuccess: onApproveTxSucess
  //       };

  //       await this.infinityWallet.branchTransactions([APPROVE_TX]);
  //     }
  //   };
  // }
}
