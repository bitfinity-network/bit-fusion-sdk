import { Principal } from '@dfinity/principal';
import * as ethers from 'ethers';
import { numberToHex } from 'viem';

import {
  Icrc2MinterIdlFactory,
  createICRC1Actor,
  createICRC2MinterActor
} from './ic';
import {
  ICRC2_MINTER_CANISTER_ID,
  ICRC2_TOKEN_CANISTER_ID,
  IS_TEST
} from './constants';
import { generateOperationId } from './tests/utils';
import { Address, Id256, Id256Factory } from './validation';
import BftBridgeABI from './abi/BFTBridge';
import WrappedTokenABI from './abi/WrappedToken';
import { Icrc1IdlFactory } from './ic';
import { Actor, HttpAgent } from '@dfinity/agent';
import { Icrc2Burn } from './canisters/icrc2-minter/icrc2-minter.did';
import { isBrowser } from './utils';

type Icrc2MinterActor = ReturnType<typeof createICRC2MinterActor>;
type Icrc1Actor = ReturnType<typeof createICRC1Actor>;

export interface IcrcBridgeOptions {
  bftBridge: any;
  baseToken: Icrc1Actor;
  icrc2Minter: Icrc2MinterActor;
  wallet: ethers.Signer;
  agent?: HttpAgent;
}

type CreateOptions = Pick<IcrcBridgeOptions, 'wallet' | 'agent'> & {
  baseTokenId?: string;
};

export class IcrcBridge {
  bftBridge: any;
  baseToken: Icrc1Actor;
  icrc2Minter: Icrc2MinterActor;
  infinityWallet?: any;
  wallet?: ethers.Signer;
  agent?: HttpAgent;

  private constructor({
    bftBridge,
    baseToken,
    icrc2Minter,
    wallet,
    agent
  }: IcrcBridgeOptions) {
    this.wallet = wallet;
    this.agent = agent;
    this.icrc2Minter = icrc2Minter;
    this.baseToken = baseToken;
    this.bftBridge = bftBridge;

    if (isBrowser()) {
      this.infinityWallet = (window as any).ic?.infinityWallet;
    }
  }

  static async create({ wallet, agent, baseTokenId }: CreateOptions) {
    const icrc2Minter = createICRC2MinterActor(
      Principal.fromText(ICRC2_MINTER_CANISTER_ID),
      agent ? { agent } : undefined
    );

    const baseToken = createICRC1Actor(
      Principal.fromText(baseTokenId || ICRC2_TOKEN_CANISTER_ID),
      agent ? { agent } : undefined
    );

    const bftBridge = await this.getBftBridgeContract(icrc2Minter, wallet);

    return new IcrcBridge({ bftBridge, baseToken, icrc2Minter, wallet, agent });
  }

  private static async getBftBridgeContract(
    icrc2Minter: Icrc2MinterActor,
    wallet: ethers.Signer
  ) {
    const [bridgeAddress] = await icrc2Minter.get_bft_bridge_contract();

    if (!bridgeAddress) {
      throw new Error('bridge address not found');
    }

    if (new Address(bridgeAddress).isZero()) {
      throw new Error('bridge contract not deployed');
    }

    return new ethers.Contract(bridgeAddress, BftBridgeABI, wallet);
  }

  get baseTokenId256(): Id256 {
    return Id256Factory.fromPrincipal(this.baseTokenId);
  }

  get baseTokenId() {
    return Actor.canisterIdOf(this.baseToken);
  }

  get icrc2MinterId() {
    return Actor.canisterIdOf(this.icrc2Minter);
  }

  async getWrappedTokenContract() {
    const wrappedTokenAddress = await this.bftBridge.getWrappedToken(
      this.baseTokenId256
    );

    if (new Address(wrappedTokenAddress).isZero()) {
      throw new Error('Invalid Address');
    }

    return new ethers.Contract(
      wrappedTokenAddress,
      WrappedTokenABI,
      this.wallet
    );
  }

  async deployBftWrappedToken(name: string, symbol: string) {
    console.log('name & symbol', name + ' ' + symbol);
    let wrappedTokenAddress = await this.bftBridge.getWrappedToken(
      this.baseTokenId256
    );

    if (wrappedTokenAddress && new Address(wrappedTokenAddress).isZero()) {
      const tx = await this.bftBridge.deployERC20(
        name,
        symbol,
        this.baseTokenId256
      );
      wrappedTokenAddress = await tx.wait(2);
    }

    return wrappedTokenAddress;
  }

  async bridgeIcrc2ToEmvc(amount: bigint, recipient: string) {
    try {
      const fee = await this.baseToken.icrc1_fee();

      const Icrc2Burn: Icrc2Burn = {
        operation_id: generateOperationId(),
        from_subaccount: [],
        icrc2_token_principal: this.baseTokenId,
        recipient_address: recipient,
        amount: numberToHex(amount)
      };

      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const onApproveTxSucess = async (res: any) => {
        console.log('approve icrc2 token res: after change', res);

        if ('Ok' in res) {
          console.log('res - icrc2', res);
        }
      };

      if (IS_TEST) {
        console.log('test is running', IS_TEST);

        const response = await this.baseToken.icrc2_approve({
          fee: [],
          memo: [],
          from_subaccount: [],
          created_at_time: [],
          amount: amount + fee * 2n,
          expected_allowance: [],
          expires_at: [],
          spender: {
            owner: Actor.canisterIdOf(this.icrc2Minter),
            subaccount: []
          }
        });

        if ('Err' in response) {
          throw new Error(
            `failed to approve tokens: ${JSON.stringify(response.Err)}`
          );
        }
        const burnResponse = await this.icrc2Minter.burn_icrc2(Icrc2Burn);
        if ('Err' in burnResponse) {
          throw new Error(
            `icrc2 minter failed to burn tokens: ${JSON.stringify(burnResponse.Err)}`
          );
        }
      } else {
        const tokenId = this.baseTokenId.toText();
        await this.infinityWallet.requestConnect({
          whitelist: [tokenId, ICRC2_MINTER_CANISTER_ID]
        });
        const APPROVE_TX = [
          {
            idl: Icrc1IdlFactory,
            canisterId: tokenId,
            methodName: 'icrc2_approve',
            args: [
              {
                fee: [],
                memo: [],
                from_subaccount: [],
                created_at_time: [],
                amount: amount + fee * 2n,
                expected_allowance: [],
                expires_at: [],
                spender: {
                  owner: Actor.canisterIdOf(this.icrc2Minter),
                  subaccount: []
                }
              }
            ],
            onSuccess: onApproveTxSucess,
            onFail: (error: any) => {
              console.log('error in burn', error);
            }
          },
          {
            idl: Icrc2MinterIdlFactory,
            canisterId: ICRC2_MINTER_CANISTER_ID,
            methodName: 'burn_icrc2',
            args: [Icrc2Burn],
            onSuccess: (res: any) => {
              console.log('burn response', res);
            },
            onFail: (error: any) => {
              console.log('error in burn', error);
            }
          }
        ];
        await this.infinityWallet.batchTransactions(APPROVE_TX);
        return 1;
      }

      // return balance;
    } catch (error) {
      console.log('error occurec', error);
    }
  }

  async bridgeEmvcToIcrc2(amount: bigint, recipient?: Principal) {
    const [bftBridgeAddress] = await this.icrc2Minter.get_bft_bridge_contract();
    if (!bftBridgeAddress) {
      throw new Error('Bft Bridge contract not registered in the icrc2 minter');
    } else if (bftBridgeAddress && new Address(bftBridgeAddress).isZero()) {
      throw new Error('Invalid Address');
    }

    const wrappedToken = await this.getWrappedTokenContract();

    await wrappedToken.approve(bftBridgeAddress, amount);

    const wrappedTokenAddress = await wrappedToken.getAddress();

    const userPrincipal = IS_TEST
      ? recipient
      : await this.infinityWallet.getPrincipal();

    const tx = await this.bftBridge.burn(
      amount,
      wrappedTokenAddress,
      Id256Factory.fromPrincipal(userPrincipal)
    );
    await tx.wait(2);
  }
}
