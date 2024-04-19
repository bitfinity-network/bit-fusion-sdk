import { Signer } from "ethers";

const getNetworkInfo = async (
  deployer: Signer
): Promise<number | undefined> => {
  // get current network
  const currentNetwork = await deployer.provider?.getNetwork();
  return currentNetwork?.chainId;
};

export default getNetworkInfo;
