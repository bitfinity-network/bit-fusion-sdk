// SPDX-License-Identifier: MIT

pragma solidity ^0.8.10;

import {Script} from "forge-std/Script.sol";
import "src/BftBridge.sol";
import {Upgrades} from "@openzeppelin-foundry-upgrades/Upgrades.sol";
import "forge-std/console.sol";

contract DeployBft is Script {
    function run() external {
        address minterAddress = vm.envAddress("MINTER_ADDRESS");
        address feeChargeAddress = vm.envAddress("FEE_CHARGE_ADDRESS");
        bool isWrappedSide = vm.envBool("IS_WRAPPED_SIDE");

        vm.startBroadcast();

        bytes memory initializeData = abi.encodeWithSelector(
            BFTBridge.initialize.selector,
            minterAddress,
            feeChargeAddress,
            isWrappedSide
        );

        address proxy = Upgrades.deployUUPSProxy(
            "BftBridge.sol:BFTBridge",
            initializeData
        );

        vm.stopBroadcast();
        console.logString("BFTBridge Proxy deployed at:");
        console.logAddress(address(proxy));

        address implementation = Upgrades.getImplementationAddress(proxy);

        console.logString("BFTBridge implementation deployed at:");
        console.logAddress(address(implementation));
    }
}
