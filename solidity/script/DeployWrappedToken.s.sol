// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "forge-std/console.sol";
import "src/BTFBridge.sol";

contract DeployWrappedToken is Script {
    function run() external {
        // Load environment variables or parameters
        address btfBridgeAddress = vm.envAddress("BTF_BRIDGE");
        string memory name = vm.envString("NAME");
        string memory symbol = vm.envString("SYMBOL");
        uint8 decimals = uint8(vm.envUint("DECIMALS"));
        bytes32 baseTokenId = vm.envBytes32("BASE_TOKEN_ID");

        // Validate inputs
        require(bytes(name).length > 0, "Token name is required");
        require(bytes(symbol).length > 0, "Token symbol is required");

        // Start broadcasting transactions
        vm.startBroadcast();

        // Get contract instance
        BTFBridge btfBridge = BTFBridge(btfBridgeAddress);

        // Deploy ERC20 token
        address tokenAddress = btfBridge.deployERC20(
            name,
            symbol,
            decimals,
            baseTokenId
        );

        // Stop broadcasting transactions
        vm.stopBroadcast();

        // Log the deployed token address
        console.log("ERC20 deployed at:", tokenAddress);
    }
}
