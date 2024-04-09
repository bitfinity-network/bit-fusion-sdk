// TODO: probably move to validation.ts
import hdkey from 'hdkey';
import { ethers } from 'ethers';
import { mnemonicToSeed } from 'bip39';
import { HttpAgent } from '@dfinity/agent';
import { Secp256k1KeyIdentity } from '@dfinity/identity-secp256k1';
import { IC_HOST, LOCAL_TEST_SEED_PHRASE, RPC_URL } from './constants';

export const fromHexString = (hexString: string) =>
  Uint8Array.from(
    hexString.match(/.{1,2}/g)!.map((byte) => parseInt(byte, 16))
  );

export const ethAddrToSubaccount = (ethAddr: string) => {
  const hex = fromHexString(ethAddr);

  const y = [];
  for (const i of hex) {
    y.push(i);
  }

  while (y.length !== 32) {
    y.push(0);
  }

  return Uint8Array.from(y);
};




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

  return new ethers.Wallet(wallet.signingKey.privateKey, provider);
};

export const getContract = (address: string, abi: any, wallet: ethers.Wallet) => {
  const contract = new ethers.Contract(address, abi, wallet);
  return contract;
};

export const wait = (ms: number) => {
  return new Promise((resolve) => setTimeout(resolve, ms));
};
