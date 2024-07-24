// SPDX-License-Identifier: MIT
pragma solidity ^0.8.7;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "src/WrappedToken.sol";
import "src/BftBridge.sol";
import "src/interfaces/IFeeCharge.sol";
import { RingBuffer } from "src/libraries/RingBuffer.sol";
import "src/abstract/TokenManager.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";

/// Make sure you add the reference contracts to the `import` statement above.
/// @custom:oz-upgrades-from src/BftBridge.sol:BFTBridge
contract BFTBridgeV2 is BFTBridge {
    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function __BridgeV2_init() public reinitializer(2) { } // Reinitialize with version 2 or higher.

    /// Creates a new ERC20 compatible token contract as a wrapper for the given `externalToken`.
    function deployERC20(
        string memory name,
        string memory symbol,
        uint8 decimals,
        bytes32 baseTokenID
    ) public returns (address) {
        require(isWrappedSide, "Only for wrapped side");
        require(_baseToWrapped[baseTokenID] == address(0), "Wrapper already exist");

        // Create the new token
        WrappedToken wrappedERC20 = new WrappedToken(name, symbol, decimals, address(this));

        _baseToWrapped[baseTokenID] = address(wrappedERC20);
        _wrappedToBase[address(wrappedERC20)] = baseTokenID;
        _wrappedTokenList.push(address(wrappedERC20));

        emit WrappedTokenDeployedEvent(name, symbol, baseTokenID, address(wrappedERC20));

        return address(wrappedERC20);
    }
}
