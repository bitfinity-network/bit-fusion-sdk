// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "src/WrappedToken.sol";
import "src/interfaces/IWrappedTokenDeployer.sol";

contract WrappedTokenDeployer is IWrappedTokenDeployer {
    /// Event emitted when a new ERC20 compatible token is deployed.
    event ERC20Deployed(address indexed token, string name, string symbol, uint8 decimals);

    /// Creates a new ERC20 compatible token contract and returns its address.
    function deployERC20(
        string memory name,
        string memory symbol,
        uint8 decimals,
        address owner
    ) external returns (address) {
        // Create the new token
        WrappedToken wrappedERC20 = new WrappedToken(name, symbol, decimals, owner);

        emit ERC20Deployed(address(wrappedERC20), name, symbol, decimals);

        return address(wrappedERC20);
    }
}
