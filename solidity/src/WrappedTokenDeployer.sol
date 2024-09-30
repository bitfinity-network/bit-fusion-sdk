// SPDX-License-Identifier: MIT
pragma solidity >=0.5.0;

import "src/WrappedToken.sol";
import "src/interfaces/IWrappedTokenDeployer.sol";

contract WrappedTokenDeployer is IWrappedTokenDeployer {
    /// Creates a new ERC20 compatible token contract and returns its address.
    function deployERC20(
        string memory name,
        string memory symbol,
        uint8 decimals,
        address owner
    ) external returns (address) {
        // Create the new token
        WrappedToken wrappedERC20 = new WrappedToken(name, symbol, decimals, owner);
        return address(wrappedERC20);
    }
}
