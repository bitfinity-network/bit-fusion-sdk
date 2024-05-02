import { isAddress } from 'web3-validator';
import { Principal } from '@dfinity/principal';
import { Buffer } from 'buffer';
import * as ethers from 'ethers';

export type Id256 = Buffer;
export type SignedMintOrder = ethers.BytesLike; //Uint8Array | number[];

export type EthAddress = `0x${string}`;

export class Id256Factory {
  chainIdFromId256(buffer: Id256): number {
    if (buffer.readUIntBE(0, 1) == 1) {
      throw Error('Needs an IC Buffer');
    }
    return buffer.readUIntBE(1, 4);
  }

  static fromPrincipal(principal: Principal): Id256 {
    const buf = Buffer.alloc(32);
    buf[0] = 0;

    const principalData = principal.toUint8Array();
    buf[1] = principalData.length;
    const prinBuffer = Buffer.from(principalData);
    buf.set(prinBuffer, 2);
    return buf;
  }

  static hexToPrincipal(hexString: string) {
    const cleanHexString = hexString.replace('0x', '');
    const buf = Buffer.from(cleanHexString, 'hex');
    const length = buf.readUInt8(1);
    const principalData = Buffer.alloc(length);
    buf.copy(principalData, 0, 2, 2 + length);
    return Principal.fromUint8Array(Uint8Array.from(principalData));
  }

  static principalToBytes32(principal: Principal): Uint8Array {
    const oldBuffer = principal.toUint8Array();

    const newBuffer = new ArrayBuffer(32);
    const buf = new Uint8Array(newBuffer);
    buf[0] = oldBuffer.length;
    buf.set(oldBuffer, 1);
    return buf;
  }

  static principalToBytes(principal: Principal): Uint8Array {
    const oldBuffer = principal.toUint8Array();
    const newBuffer = new ArrayBuffer(oldBuffer.length + 1);
    const buf = new Uint8Array(newBuffer);
    buf[0] = oldBuffer.length;
    buf.set(oldBuffer, 1);
    return buf;
  }

  static fromAddress(input: AddressWithChainID): Id256 {
    const buf = Buffer.alloc(32); // Create a buffer with 32 bytes
    // Set the first byte to EVM_ADDRESS_MARK (0x01 in this example)
    buf[0] = 0x01;

    // Convert the chainId to big-endian and add it to the buffer
    const chainIdBuf = Buffer.alloc(4);
    chainIdBuf.writeUInt32BE(Number(input.getChainID()));
    chainIdBuf.copy(buf, 1, 0, 4);

    // Convert the address to bytes and add it to the buffer
    const addressBuf = input.addressAsBuffer();
    addressBuf.copy(buf, 5);
    return buf;
  }

  static from(input: AddressWithChainID | Principal): Id256 {
    if (typeof input == typeof AddressWithChainID) {
      return this.fromAddress(input as AddressWithChainID);
    } else {
      return this.fromPrincipal(input as Principal);
    }
  }
}

export class Address {
  private address: string;

  public getAddress(): string {
    return this.address;
  }

  public addressAsBuffer(): Id256 {
    return Buffer.from(this.address.replace('0x', ''), 'hex');
  }

  public isZero(): boolean {
    return /^0x0+$/.test(this.address);
  }

  constructor(address: string) {
    this.address = address;

    if (!isAddress(this.addressAsBuffer())) {
      console.log(address);
      throw Error('Not a valid Address');
    }
  }
}

export class AddressWithChainID extends Address {
  private chainID: number;

  public getChainID(): number {
    return this.chainID;
  }

  constructor(address: string, chainID: number) {
    super(address);
    this.chainID = chainID;
  }
}
