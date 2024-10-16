// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "src/FeeCharge.sol";

contract DeployFeeCharge is Script {
    function run() external {
        address[] memory bridges = vm.envAddress("BRIDGES", ",");
        address expectedAddress = vm.envAddress("EXPECTED_ADDRESS");

        vm.startBroadcast();

        FeeCharge feeCharge = new FeeCharge(bridges);
        address feeChargeAddress = address(feeCharge);

        if (
            expectedAddress != address(0) && feeChargeAddress != expectedAddress
        ) {
            revert("Fee charge address does not match the expected address");
        }

        console.log("Fee charge address:", feeChargeAddress);

        vm.stopBroadcast();
    }
}
