// SPDX-License-Identifier: MIT
pragma solidity >=0.5.0;

// Allows to depsit/withdraw native tokens and use the deposit to charge fee.
interface IFeeCharge {
    // Deposit `msg.value` amount of native token to `msg.sender` address.
    // Returns user's balance after the operation.
    function nativeTokenDeposit() external payable returns (uint256 balance);

    // Withdraw the amount of native token to user's address.
    // Returns user's balance after the operation.
    function nativeTokenWithdraw(
        uint256 amount
    ) external payable returns (uint256 balance);

    // Returns user's native token deposit balance.
    function nativeTokenBalance(
        address user
    ) external view returns (uint256 balance);

    // Charge the given amount of fee from the user.
    // Require the user to have enough native token deposit balance.
    function chargeFee(address from, address payable to, uint256 amount) external;

    /// Function to check if fee charge operation can be performed.
    function canPayFee(address from, uint256 amount) external view returns (bool);
}
