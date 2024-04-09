import hdkey from 'hdkey';
import { ethers } from 'ethers';
import { mnemonicToSeed } from 'bip39';
import { HttpAgent } from '@dfinity/agent';
import { Secp256k1KeyIdentity } from '@dfinity/identity-secp256k1';
import { IC_HOST, LOCAL_TEST_SEED_PHRASE, RPC_URL } from '../constants';

export const createAnnonAgent = () => {
  return new HttpAgent({
    host: IC_HOST
  });
};

export const createAgent = () => {
  const identity = identityFromSeed(LOCAL_TEST_SEED_PHRASE);

  const agent = new HttpAgent({
    host: IC_HOST,
    identity
  });


  return agent;
};

export const identityFromSeed = async (
  phrase: string
): Promise<Secp256k1KeyIdentity> => {
  const seed = await mnemonicToSeed(phrase);
  const root = hdkey.fromMasterSeed(seed);
  const addrnode = root.derive("m/44'/223'/0'/0/0");

  const id = Secp256k1KeyIdentity.fromSecretKey(addrnode.privateKey);
  return id;
};

export const generateOperationId = () => {
  const timestamp = Date.now();
  const randomNum = Math.floor(Math.random() * 0x100000000);
  const uniqueId = (timestamp + randomNum) % 0x100000000;
  return uniqueId;
};

export const generateEthWallet = () => {
  const wallet = ethers.Wallet.fromPhrase(LOCAL_TEST_SEED_PHRASE);
  const provider = new ethers.JsonRpcProvider(RPC_URL);

  return wallet.connect(provider);

};

export const getContract = (address: string, abi: any) => {
  const provider = new ethers.JsonRpcProvider(RPC_URL);
  const contract = new ethers.Contract(address, abi, provider);
  return contract;
};

export const wait = (ms: number) => {
  return new Promise((resolve) => setTimeout(resolve, ms));
};


// Function to mint tokens
export async function mintNativeToken(toAddress: string, amount: string) {
  const response = await fetch(RPC_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: "67",
      method: "ic_mintNativeToken",
      params: [toAddress, amount],
    }),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
}