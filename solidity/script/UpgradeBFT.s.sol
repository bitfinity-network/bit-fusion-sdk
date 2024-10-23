// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "@openzeppelin/foundry-upgrades/Upgrades.sol";
import { Options } from "@openzeppelin/foundry-upgrades/Options.sol";

/**
 * @title PrepareUpgradeScript
 * @dev Step 1: Deploys the new implementation contract.
 * Running this script will deploy the updated contract (BFTBridgeV2).
 */
contract PrepareUpgrade is Script {
    address proxyAddress;
    address newImplementationAddress;

    function setUp() public {
        // Read the proxy address from the environment variables
        proxyAddress = vm.envAddress("PROXY_ADDRESS");
    }

    function run() external {
        setUp();

        console.log("Starting upgrade preparation...");

        // Start broadcasting to the network
        vm.startBroadcast();

        // Deploy the new implementation contract (BFTBridgeV2)
        string memory contractName = "BftBridge.sol:BFTBridgeV2";
        Options memory opts;
        newImplementationAddress = Upgrades.prepareUpgrade(contractName, opts);

        // Stop broadcasting
        vm.stopBroadcast();

        // Retrieve and log the address of the new implementation
        console.log("New implementation deployed at:", newImplementationAddress);

        // Optionally, store the implementation address for the next steps
        vm.writeLine(".implementation_address", vm.toString(newImplementationAddress));

        console.log("Upgrade preparation completed.");
    }
}

/**
 * @title AddNewImplementationScript
 * @dev Step 2: Adds the new implementation to the proxy contract's allowed implementations.
 * This script adds the bytecode hash of the new implementation to the allowed list.
 */
contract AddNewImplementation is Script {
    address proxyAddress;
    address newImplementationAddress;

    function setUp() public {
        // Read the addresses from the environment variables
        proxyAddress = vm.envAddress("PROXY_ADDRESS");
        string memory newImp = vm.readFile(".implementation_address");
        newImplementationAddress = address(bytes20(bytes(newImp)));
    }

    function run() external {
        setUp();

        console.log("Adding new implementation...");

        // Start broadcasting to the network
        vm.startBroadcast();

        // Get the deployed bytecode and compute its hash
        bytes memory deployedBytecode = address(newImplementationAddress).code;
        bytes32 bytecodeHash = keccak256(deployedBytecode);

        // Interface to interact with the proxy contract (assuming it's an Ownable UUPS proxy)
        IBFTBridge proxyContract = IBFTBridge(proxyAddress);

        // Add the new implementation's bytecode hash to allowed implementations
        proxyContract.addAllowedImplementation(bytecodeHash);

        // Stop broadcasting
        vm.stopBroadcast();

        console.log("New implementation added successfully.");
    }
}

/**
 * @title UpgradeProxyScript
 * @dev Step 3: Upgrades the proxy contract to the new implementation.
 * This script performs the upgrade to point the proxy to the new implementation.
 */
contract UpgradeProxy is Script {
    address proxyAddress;
    address newImplementationAddress;

    function setUp() public {
        // Read the addresses from the environment variables
        proxyAddress = vm.envAddress("PROXY_ADDRESS");
        newImplementationAddress = vm.envAddress("NEW_IMPLEMENTATION_ADDRESS");
    }

    function run() external {
        setUp();

        console.log("Upgrading proxy...");

        // Start broadcasting to the network
        vm.startBroadcast();

        // Interface to interact with the proxy contract
        IBFTBridge proxyContract = IBFTBridge(proxyAddress);

        // Prepare the initialization data
        bytes memory initData = abi.encodeWithSignature("__BridgeV2_init()");

        // Upgrade the proxy to the new implementation and call the initializer
        proxyContract.upgradeToAndCall(newImplementationAddress, initData);

        // Stop broadcasting
        vm.stopBroadcast();

        console.log("Proxy upgraded successfully.");
    }
}

/**
 * @dev Interface to interact with the BFTBridge proxy contract.
 * Include the functions needed for the upgrade process.
 */
interface IBFTBridge {
    function addAllowedImplementation(
        bytes32 bytecodeHash
    ) external;

    function upgradeToAndCall(address newImplementation, bytes memory data) external;
}
