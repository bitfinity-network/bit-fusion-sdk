// SPDX-License-Identifier: MIT
pragma solidity ^0.8.7;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "src/WrappedToken.sol";
import "src/BftBridge.sol";
import "src/interfaces/IFeeCharge.sol";
import { RingBuffer } from "src/libraries/RingBuffer.sol";
import "src/abstract/TokenManager.sol";
import "@openzeppelin-contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin-contracts-upgradeable/access/OwnableUpgradeable.sol";
import "@openzeppelin-contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin-contracts-upgradeable/utils/PausableUpgradeable.sol";

/// Make sure you add the reference contracts to the `import` statement above.
/// @custom:oz-upgrades-from src/BftBridge.sol:BFTBridge
contract BFTBridgeV2 is BFTBridge {
    // Hello World
    function helloWorld(string memory name) public pure returns (string memory) {
        return string(abi.encodePacked("Hello, ", name, "!"));
    }

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function __BridgeV2_init() public reinitializer(2) { } // Reinitialize with version 2 or higher.
}
