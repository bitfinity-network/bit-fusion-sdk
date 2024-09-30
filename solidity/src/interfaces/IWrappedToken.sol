// SPDX-License-Identifier: MIT
pragma solidity >=0.5.0;

/// Additional owner-only methods for ERC-20 compatible WrappedToken.
interface IWrappedToken {
    /// Allows an owner to change other wallet allowance.
    function approveByOwner(address from, address spender, uint256 value) external returns (bool);

    // Allows an owner to update token name, symbol and decimals if needed.
    function setMetaData(bytes32 name_, bytes16 symbol_, uint8 decimals_) external;
}
