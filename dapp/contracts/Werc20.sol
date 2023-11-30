// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.19;
import "../node_modules/@openzeppelin/contracts/interfaces/IERC20.sol";

contract WERC20 {

    uint256 public counter;
    IERC20 token;

    constructor(IERC20 _token) {
        token = _token;
    }
    function transferFrom(address from, address to, uint256 amount) public {
        require(token.transferFrom(from, to, amount), "Transfer failed");
        counter++;
    }
}