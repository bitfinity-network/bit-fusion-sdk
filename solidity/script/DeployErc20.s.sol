// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "src/WrappedToken.sol";

contract DeployErc20 is Script {
    function run() external {
        string memory name = vm.envString("NAME");
        string memory symbol = vm.envString("SYMBOL");
        address owner = msg.sender;

        vm.startBroadcast();

        WrappedToken token = new WrappedToken(name, symbol, 18, owner);
        console.log("Token address: ", address(token));

        token.transfer(owner, 1000000000000000000000000);
        console.log("Transferred many tokens to the owner");

        vm.stopBroadcast();
    }
}
