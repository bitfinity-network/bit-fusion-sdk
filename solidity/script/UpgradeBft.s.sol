// SPDX-License-Identifier: MIT

pragma solidity ^0.8.10;

import {Script} from "forge-std/Script.sol";
import "src/BftBridge.sol";
import {Upgrades} from "@openzeppelin-foundry-upgrades/Upgrades.sol";
import "forge-std/console.sol";

contract UpgradeBft is Script {
    function run() external {
        address minterAddress = vm.envAddress("MINTER_ADDRESS");
        address feeChargeAddress = vm.envAddress("FEE_CHARGE_ADDRESS");
        bool isWrappedSide = vm.envBool("IS_WRAPPED_SIDE");
        address proxyAddress = vm.envAddress("PROXY_ADDRESS");

        vm.startBroadcast();

        // Rename the contract version
        string memory newImplementation = "BftBridge.sol:BFTBridgeVx";

        bytes memory initializeData =
            abi.encodeWithSelector(BFTBridge.initialize.selector, minterAddress, feeChargeAddress, isWrappedSide);

        // Upgrade the proxy to the new implementation
        Upgrades.upgradeProxy(proxyAddress, newImplementation, initializeData);

        vm.stopBroadcast();
        console.logString("BFTBridge Proxy upgraded at:");
        console.logAddress(proxyAddress);
        address newImplementationAddress = Upgrades.getImplementationAddress(proxyAddress);
        console.logString("New BFTBridge implementation deployed at:");
        console.logAddress(address(newImplementationAddress));
    }
}
