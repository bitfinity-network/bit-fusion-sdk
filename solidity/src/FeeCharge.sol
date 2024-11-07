// SPDX-License-Identifier: MIT
pragma solidity ^0.8.7;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "src/interfaces/IFeeCharge.sol";
import { RingBuffer } from "src/libraries/RingBuffer.sol";

contract FeeCharge is IFeeCharge {
    // Mapping from user address to amount of native tokens on his deposit.
    mapping(address => uint256) private _userBalance;

    // Addresses allowed to charge fee from users.
    mapping(address => bool) private _canChargeFee;

    constructor(
        address[] memory canChargeFee
    ) {
        uint256 length = canChargeFee.length;
        for (uint256 i = 0; i < length; i++) {
            address approved = canChargeFee[i];
            _canChargeFee[approved] = true;
        }
    }

    // Deposit `msg.value` amount of native token to `msg.sender` address.
    // Returns user's balance after the operation.
    function nativeTokenDeposit() external payable returns (uint256 balance) {
        address to = msg.sender;
        require(to != address(0), "expected non-zero to address");

        balance = _userBalance[to];
        balance += msg.value;
        _userBalance[to] = balance;
    }

    // Withdraw the amount of native token to user's address.
    // Returns user's balance after the operation.
    function nativeTokenWithdraw(
        uint256 amount
    ) external payable returns (uint256 balance) {
        require(amount > 0, "failed to withdraw zero amount");
        address to = msg.sender;

        balance = _userBalance[to];
        require(balance >= amount, "insufficient balance to withdraw");
        balance -= amount;
        _userBalance[to] = balance;
        payable(to).transfer(amount);
    }

    // Returns user's native token deposit balance.
    function nativeTokenBalance(
        address user
    ) external view returns (uint256 balance) {
        if (user == address(0)) {
            user = msg.sender;
        }
        balance = _userBalance[user];
    }

    // Take the given amount of fee from the user.
    // Require the user to have enough native token balance and approval for senderID.
    function chargeFee(address from, address payable to, uint256 amount) external {
        require(_canChargeFee[msg.sender], "fee charger is not present in allow list");
        uint256 balance = _userBalance[from];
        require(balance >= amount, "insufficient balance to pay fee");

        uint256 newBalance = balance - amount;
        _userBalance[from] = newBalance;
        to.transfer(amount);
    }

    /// Function to check if fee charge operation can be performed.
    function canPayFee(address payer, uint256 amount) external view returns (bool) {
        /// Check if the msg.sender is able to charge fee.
        if (!_canChargeFee[msg.sender]) {
            return false;
        }

        /// Check if the payer have enough balance.
        uint256 balance = _userBalance[payer];
        if (balance < amount) {
            return false;
        }

        return true;
    }
}
