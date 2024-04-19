import hdkey from 'hdkey';
import { ethers } from 'ethers';
import { mnemonicToSeed } from 'bip39';
import { HttpAgent } from '@dfinity/agent';
import { exec } from 'child_process';
import { Secp256k1KeyIdentity } from '@dfinity/identity-secp256k1';
import {
  CHAIN_ID,
  IC_HOST,
  LOCAL_TEST_SEED_PHRASE,
  RPC_URL
} from '../constants';

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

export const getProvider = () => {
  return new ethers.JsonRpcProvider(RPC_URL, {
    name: 'Bitfinity',
    chainId: Number(CHAIN_ID)
  });
};

export const generateWallet = () => {
  const wallet = ethers.Wallet.fromPhrase(LOCAL_TEST_SEED_PHRASE);
  // const wallet = new ethers.Wallet(
  //   '0xe96e898e18631ef63c31ae9349a332cced5075e04fd5bcf4e212b6ecea699ee3'
  // );
  const provider = getProvider();

  return wallet.connect(provider);
};

export const getContract = (address: string, abi: any) => {
  const provider = getProvider();
  const contract = new ethers.Contract(address, abi, provider);
  return contract;
};

export const wait = (ms: number) => {
  return new Promise((resolve) => setTimeout(resolve, ms));
};

export const generateBitcoinToAddress = (address: string) => {
  const command = `~/bitcoin-25.0/bin/bitcoin-cli -conf="/Users/andyosei/Desktop/projects/infinity_swap/ckERC20/src/create_bft_bridge_tool/bitcoin.conf" generatetoaddress 1 "${address}"`;

  exec(command, (error, stdout, stderr) => {
    if (error) {
      console.error(`exec error: ${error}`);
      return;
    }
    if (stderr) {
      console.error(`stderr: ${stderr}`);
      return;
    }
    console.log(`stdout: ${stdout}`);
  });
};
