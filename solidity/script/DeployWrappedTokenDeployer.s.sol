// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "src/WrappedTokenDeployer.sol";

contract DeployWrappedTokenDeployer is Script {
    function run() external {
        vm.startBroadcast();

        WrappedTokenDeployer wrappedTokenDeployer = new WrappedTokenDeployer();
        address wrappedTokenDeployerAddress = address(wrappedTokenDeployer);

        console.log(
            "WrappedTokenDeployer address:",
            wrappedTokenDeployerAddress
        );

        vm.stopBroadcast();
    }
}
