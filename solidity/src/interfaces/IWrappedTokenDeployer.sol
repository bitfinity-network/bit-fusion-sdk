// SPDX-License-Identifier: MIT
pragma solidity >=0.5.0;

/// Manage wrapped token deployment.
interface IWrappedTokenDeployer {
    /// Creates a new ERC20 compatible token contract as a wrapper for the given `externalToken`.
    function deployERC20(
        string memory name,
        string memory symbol,
        uint8 decimals,
        address owner
    ) external returns (address);
}
