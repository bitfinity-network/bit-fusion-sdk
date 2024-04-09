import { expect, test } from 'vitest';
import { createAgent, generateEthWallet, wait, mintNativeToken } from './utils';
import { IcrcBridge } from '../icrc';
import { Id256Factory, Address } from '../validation';
import { Principal } from '@dfinity/principal';
import { ICRC2_TOKEN_CANISTER_ID, ICRC2_MINTER_CANISTER_ID } from '../constants';
import {createAgent} from "../utils";
import {ICRC1, ICRC2Minter} from "../ic"
import { getContract } from 'viem';
import { ethers } from 'ethers';

import WrappedTokenABI from "../abi/WrappedToken";
(BigInt as any).prototype.toJSON = function () {
  return this.toString();
};

test('bridge icrc2 token to evm', async () => {
  const amount = 100000;

  

  const agent = await createAgent();
  console.log("This is the principal:", (await agent.getPrincipal()).toString());

  //const agent = createAgent();
  const wallet = generateEthWallet();

  const walletAddress = await wallet.getAddress();

  const balance = await wallet.provider?.getBalance(walletAddress);
  
  expect(balance).toBeGreaterThan(0);


  const icrcBridge = new IcrcBridge({ wallet});

  let evm_address_res = await ICRC2Minter.get_minter_canister_evm_address();



  const fee = await ICRC1?.icrc1_fee();
  const _ = await ICRC1?.icrc2_approve({
             fee: [],
             memo: [],
             from_subaccount: [],
             created_at_time: [],
             amount: BigInt(amount) + fee + fee!,
             expected_allowance: [],
             expires_at: [],
             spender: {
               owner: Principal.fromText(ICRC2_MINTER_CANISTER_ID),
               subaccount: []
             }
  });


  
  const icrc_principal = Principal.fromText(ICRC2_TOKEN_CANISTER_ID);
  const id = Id256Factory.fromPrincipal(icrc_principal);

  const icrcBridgeContract = await icrcBridge.getBftBridgeContract();

  const wrappedTokenAddress = await icrcBridgeContract.getWrappedToken(id);
 
  if (new Address(wrappedTokenAddress).isZero()) {
    await icrcBridgeContract.deployERC20(
      'AUX',
      'AUX',
      Id256Factory.fromPrincipal(Principal.fromText(ICRC2_TOKEN_CANISTER_ID))
    );
  }

  //TODO: take care of the fee/ decimals?? 
  // 60 or above fails for some reason? 
  let resp = await icrcBridge.bridgeIcrc2(amount, wallet.address);
  console.log("this si the response", resp);

  await wait(10000);

  const token = new ethers.Contract(
         wrappedTokenAddress,
         WrappedTokenABI,
         wallet.provider)

  const ERC20balance = await token.balanceOf(wallet.address);

  if ('Ok' in evm_address_res) {
    const evm_address = evm_address_res.Ok;
    const minterERC20balance = await token.balanceOf(evm_address);

    const balance = await wallet.provider?.getBalance(evm_address);
    console.log("balance evm address", balance, minterERC20balance)
  } 

  console.log(ERC20balance);

  // expect(balance).toBe(amount);
  expect(ERC20balance).toBeGreaterThan(0);
}, 25000);
