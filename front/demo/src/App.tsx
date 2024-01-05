import abi from 'human-standard-token-abi';
import abiDecoder from 'abi-decoder';
import { ethers, BrowserProvider, Contract } from 'ethers';
import { MetaMaskInpageProvider } from '@metamask/providers';
import { useState } from 'react';

abiDecoder.addABI(abi);

export const connect = async () => {
  try {
    const result = await window.ethereum.request({
      method: 'eth_requestAccounts'
    });

    console.log(result);
  } catch (err) {
    console.log(err);
  }
};

let provider: ethers.BrowserProvider;
let signer: ethers.JsonRpcSigner;

const get = async () => {
  if (provider) {
    return {
      provider,
      signer
    };
  }

  try {
    provider = new BrowserProvider(window.ethereum as any);
    signer = await provider.getSigner();
  } catch (err) {
    throw err;
  }

  return {
    provider,
    signer
  };
};

export const balance = async (contract: string, address: string) => {
  await get();

  try {
    const token = new Contract(contract, abi, signer);

    const result = await token.balanceOf!(address);

    console.log(result);
  } catch (err) {
    console.log(err);
  }
};

export const send = async (contract: string, address: string) => {
  await get();

  try {
    const token = new Contract(contract, abi, signer);

    const from = address;
    const to = '0xd3103da8D5AAf016dA81EBF21b1A4AB9851C529E';
    const amount = BigInt(1 * 1000000000000000000);

    const approval = await token.approve(
      to,
      amount /*, {gasLimit: BigInt(10000000000)}*/
    );

    console.log(approval);

    const transfer = await token.transferFrom(from, to, amount);

    console.log(transfer);
  } catch (err) {
    console.log(err);
  }
};

async function mock() {
  window.ethereum = new Proxy(window.ethereum, {
    get: function (item, property) {
      const prop = item[property as keyof MetaMaskInpageProvider];

      if (typeof prop === 'function') {
        return function (...args: any[]) {
          if (args[0]?.method) {
            if (
              ['eth_sendTransaction', 'eth_call', 'eth_estimateGas'].includes(
                args[0].method
              ) &&
              Array.isArray(args[0].params) &&
              args[0].params[0].data
            ) {
              console.log(
                args[0].method,
                abiDecoder.decodeData(args[0].params[0].data),
                args[0].params
              );
            } else {
              console.log(args[0].method, args[0].params);
            }
          } else {
            console.log(property, args);
          }

          return (prop as any)(...args);
        };
      }

      return prop;
    }
  });
}

mock();

function Button({ children, onClick }: { onClick: () => void; children: any }) {
  return (
    <button
      onClick={onClick}
      className="flex justify-center rounded-md bg-indigo-600 px-3 py-1.5 text-sm font-semibold leading-6 text-white shadow-sm hover:bg-indigo-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
    >
      {children}
    </button>
  );
}

function Input({
  value,
  onChange
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex rounded-md shadow-sm ring-1 ring-inset ring-gray-300 focus-within:ring-2 focus-within:ring-inset focus-within:ring-indigo-600 sm:max-w-md">
      <input
        type="text"
        className="block flex-1 border-0 bg-transparent py-1.5 pl-1 text-gray-900 placeholder:text-gray-400 focus:ring-0 sm:text-sm sm:leading-6"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

function App() {
  const [contract, setContract] = useState(
    '0x8A17F043C709ef83703780f050008666f02557B8'
  );
  const [address, setAddress] = useState(
    '0xFA65FdC6785688F7507bDA0f6119C80e1e23ecF7'
  );

  return (
    <div className="p-5">
      <div className="mb-5">
        <h1>Testing the ETH mocks</h1>

        <p>First connect the wallet.</p>

        <p>Open the console, and click balance or send.</p>

        <p>In the console you will see the intercepted messages.</p>
      </div>

      <div className="mb-3">
        <Input value={contract} onChange={setContract} />
        <Input value={address} onChange={setAddress} />
      </div>

      <div className="mb-1">
        <Button onClick={connect}>connect</Button>
      </div>
      <div className="mb-1">
        <Button onClick={() => balance(contract, address)}>balance</Button>
      </div>
      <div className="mb-1">
        <Button onClick={() => send(contract, address)}>send</Button>
      </div>
    </div>
  );
}

export default App;
