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

  const provider = getProvider();

  return wallet.connect(provider);
};

export const randomWallet = () => {
  const wallet = ethers.Wallet.createRandom();

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

export const execCmd = (cmd: string): Promise<string> => {
  return new Promise((resolve, reject) => {
    exec(cmd, (err, stdout) => {
      if (err) {
        return reject(err);
      } else if (stdout) {
        resolve(stdout);
      }
    });
  });
};

export const execBitcoinCmd = (cmd: string) => {
  return execCmd(`${process.env.BITCOIN_CMD} ${cmd}`);
};

export const execOrdCmd = (cmd: string) => {
  return execCmd(`${process.env.ORD_CMD} ${cmd}`);
};

export const execOrdSend = async (address: string, runeName: string) => {
  try {
    const response = await execOrdCmd(
      `wallet --server-url http://0.0.0.0:8000 send --fee-rate 10 ${address} 10:${runeName}`
    );

    const result = JSON.parse(response);

    return result.txid;
  } catch (_) {
    return null;
  }
};

export const execOrdReceive = async () => {
  try {
    const response = await execOrdCmd(
      `wallet --server-url http://0.0.0.0:8000 receive`
    );

    const result = JSON.parse(response);

    return result.addresses[0];
  } catch (_) {
    return null;
  }
};

export async function mintNativeToken(toAddress: string, amount: string) {
  const response = await fetch(process.env.RPC_URL!, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: '67',
      method: 'ic_mintNativeToken',
      params: [toAddress, amount]
    })
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
}
