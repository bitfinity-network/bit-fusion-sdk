// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "src/WrappedTokenDeployer.sol";

contract DeployWrappedTokenDeployer is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(deployerPrivateKey);

        WrappedTokenDeployer wrappedTokenDeployer = new WrappedTokenDeployer();
        address wrappedTokenDeployerAddress = address(wrappedTokenDeployer);

        console.log(
            "WrappedTokenDeployer address:",
            wrappedTokenDeployerAddress
        );

        vm.stopBroadcast();
    }
}
