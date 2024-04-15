// When running the script with `npx hardhat run <script>` you'll find the Hardhat
// Runtime Environment's members available in the global scope.
import { ethers } from "hardhat";

import writeLogFile from "../helpers/write-files";
import web3 from "web3";
import { BigNumber } from "ethers";

const PATTERN = /[\u0000-\u0019]/g;
// list of all EVMC token contracts --> contracts/EVMCTokens.sol
const EVMC_TOKENS: string[] = [
  "Cashium",
  "Coinaro",
  "Coinverse",
  "Coinovation",
  "Intellicoin",
  "Incoingnito",
  "Arcoin",
  "Coinicious",
];

export const AMOUNT_TO_CLAIM: number = 100 * 10 ** 18;

interface IToken {
  address: string;
  name: string;
  symbol: string;
  decimals: number;
}

async function main() {
  const addresses: { [key: string]: IToken } = {};
  // create transaction signer
  const deployers = await ethers.getSigners();
  const deployer = deployers[0];
  const contractOwner = deployer.address;

  console.log("Deploying contracts with the account:", contractOwner);



  /*
  Deploy faucet contract
   */
  const FaucetFactory = await ethers.getContractFactory(
    "Faucet",
    deployer
  );
  const amountToClaim = BigNumber.from(AMOUNT_TO_CLAIM.toString());

  const faucetDeploy = FaucetFactory.getDeployTransaction(
    contractOwner,
    amountToClaim
  );
  const faucetTx = await deployer.sendTransaction({
    nonce: await deployer.getTransactionCount(),
    ...faucetDeploy,
  });
  const faucet = await faucetTx.wait();
  const faucetAddress = faucet.contractAddress;
  console.log("Deployed Faucet to: ", faucetAddress);

  // write file to json
  const faucetInput = { date: new Date(), faucetAddress };
  const faucetFile: string = "faucetAddress.json";
  writeLogFile(faucetFile, faucetInput);

  /*
   deploy token contracts
   */
  const deployTokens = async () => {
    for (const t of EVMC_TOKENS) {
      // deploy token contract
      const Token = await ethers.getContractFactory(t, deployer);
      const tokenDeploy = Token.getDeployTransaction();
      const tokenTx = await deployer.sendTransaction({
        nonce: await deployer.getTransactionCount(),
        ...tokenDeploy,
      });
      const token = await tokenTx.wait();

      const tokenContract = await ethers.getContractAt(
        t,
        token.contractAddress
      );
      const nameTx = await tokenContract.populateTransaction.name();
      const name = await deployer.call({
        nonce: await deployer.getTransactionCount(),
        ...nameTx,
      });

      const symbolTx = await tokenContract.populateTransaction.symbol();
      const symbol = await deployer.call({
        nonce: await deployer.getTransactionCount(),
        ...symbolTx,
      });

      addresses[t] = {
        address: token.contractAddress,
        name: web3.utils.hexToAscii(name).replace(PATTERN, "").trim(),
        symbol: web3.utils.hexToAscii(symbol).replace(PATTERN, "").trim(),
        decimals: 18,
      };
      console.log(`Deployed ${t} to: ${token.contractAddress}`);

      const initialContract = await ethers.getContractAt(
        t,
        token.contractAddress
      );
      const initialSupply =
        await initialContract.populateTransaction.INITIAL_SUPPLY();

      const initialSupplyTx = await deployer.call({
        nonce: await deployer.getTransactionCount(),
        ...initialSupply,
      });

      // convert from hex to decimal
      const initialSupplyDec =
        ethers.BigNumber.from(initialSupplyTx).toString();

      console.log(`Initial supply of ${t} is ${initialSupplyDec}`);

      const spendableAmount = ethers.utils.formatUnits(initialSupplyDec, 18);
      // approve faucet contract to spend tokens
      console.log(
        `Approving faucet contract to spend ${spendableAmount} of ${t}...`
      );

      const approveTx = await tokenContract.populateTransaction.approve(
        faucetAddress,
        initialSupplyDec
      );

      const approveResponse = await deployer.sendTransaction({
        nonce: await deployer.getTransactionCount(),
        ...approveTx,
      });
      await approveResponse.wait();

      console.log(`Approved faucet contract to spend ${t} tokens`);
    }
  };
  await deployTokens();
  console.log(
    `Deployed ${EVMC_TOKENS.length} tokens to the Bitfinity network!`
  );

  // write file to json
  const tokenInput = {
    date: new Date(),
    tokens: addresses,
  };

  const tokenFile: string = "tokenAddresses.json";
  writeLogFile(tokenFile, tokenInput);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
