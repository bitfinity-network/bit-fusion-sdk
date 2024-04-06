import { expect, test } from 'vitest';
import { Principal } from '@dfinity/principal';
import { createICRC1Actor, createICRC2MinterActor } from '..';
import {
  createAgent,
  generateOperationId,
  generateWallet,
  getContract,
  wait
} from './utils';
import { Address, Id256Factory } from '../validation';
import canisters from '../../../.dfx/local/canister_ids.json';
import BftBridgeABI from '../abi/BFTBridge.json';
import WrappedTokenABI from '../abi/WrappedToken.json';

BigInt.prototype.toJSON = function () {
  return this.toString();
};

const ICRC2_MINTER_CANISTER_ID = Principal.fromText(
  canisters['icrc2-minter'].local
);
const ICRC2_TOKEN_CANISTER_ID = Principal.fromText(canisters.token2.local);

test('bridge icrc2 token to evm', async () => {
  const amount = BigInt(100);

  const wallet = generateWallet();

  const agent = createAgent();

  const token = createICRC1Actor(ICRC2_TOKEN_CANISTER_ID, { agent });

  const tokenFee = await token.icrc1_fee();

  const response = await token.icrc2_approve({
    fee: [tokenFee],
    memo: [],
    from_subaccount: [],
    created_at_time: [],
    amount: amount,
    expected_allowance: [],
    expires_at: [],
    spender: {
      owner: ICRC2_MINTER_CANISTER_ID,
      subaccount: []
    }
  });

  if ('Err' in response) {
    expect.fail(
      `Failed to approve the ${amount} ICRC2 token: ${JSON.stringify(response.Err)}`
    );
  }

  const icrc2Minter = createICRC2MinterActor(ICRC2_MINTER_CANISTER_ID, {
    agent
  });

  const burnIcrc2Response = await icrc2Minter.burn_icrc2({
    operation_id: generateOperationId(),
    from_subaccount: [],
    icrc2_token_principal: ICRC2_TOKEN_CANISTER_ID,
    recipient_address: wallet.address,
    amount: amount.toString()
  });

  if ('Err' in burnIcrc2Response) {
    expect.fail(
      `ICRC2 Minter failed to burn the ${amount} ICRC2 tokens: ${JSON.stringify(burnIcrc2Response.Err)}`
    );
  }

  const [bridgeAddress] = await icrc2Minter.get_bft_bridge_contract();
  console.log('bridgeAddress:', bridgeAddress);
  if (!bridgeAddress) {
    expect.fail('Failed to get bft bridge contract address');
  }

  const bftBridgeContract = getContract(bridgeAddress!, BftBridgeABI.abi);

  const baseTokenPrincipal = ICRC2_TOKEN_CANISTER_ID;
  console.log('baseTokenPrincipal:', baseTokenPrincipal.toText());
  const wrappedToken = await bftBridgeContract.getWrappedToken(
    Id256Factory.fromPrincipal(baseTokenPrincipal)
  );
  console.log('wrappedToken:', wrappedToken);

  const wrappedTokenAddress = new Address(wrappedToken);
  if (wrappedTokenAddress.isZero()) {
    expect.fail('Invalid wrapped token address');
  }

  const wrappedTokenContract = getContract(
    wrappedTokenAddress.getAddress(),
    WrappedTokenABI.abi
  );

  await wait(2000);

  console.log('wallet address:', wallet.address);
  const balance = await wrappedTokenContract.balanceOf(wallet.address);

  expect(balance).toBe(amount);
}, 15000);
