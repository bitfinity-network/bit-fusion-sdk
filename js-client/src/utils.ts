import { Buffer } from 'buffer';

export const fromHexString = (hexString: string) =>
  Uint8Array.from(
    hexString.match(/.{1,2}/g)!.map((byte) => parseInt(byte, 16))
  );

export const ethAddrToSubaccount = (ethAddr: string) => {
  ethAddr = ethAddr.replace(/^0x/, '');

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

export const encodeBtcAddress = (address: string) => {
  return Buffer.from(address, 'utf8').toString('hex');
};

export const isBrowser = () => {
  return (
    typeof window !== 'undefined' && typeof window.document !== 'undefined'
  );
};
