// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import 'forge-std/Script.sol';
import 'src/BFTBridge.sol';

/**
 * @title PauseUnpauseScript
 * @dev This script allows pausing and unpausing of the BFTBridge contract
 *
 * To run this script:
 * forge script script/PauseUnpause.s.sol:PauseUnpauseScript --rpc-url <your_rpc_url> --private-key <your_private_key> --broadcast
 *
 * Add the following arguments to pause or unpause:
 * --sig "run(address,bool)" <contract_address> <true_to_pause_false_to_unpause>
 * !!! MAKE SURE THE OWNER IS THE SAME AS THE DEPLOYER
 */
contract PauseUnpauseScript is Script {
    function setUp() public {}

    function run(address contractAddress, bool shouldPause) public {
        uint256 deployerPrivateKey = vm.envUint('PRIVATE_KEY');
        vm.startBroadcast(deployerPrivateKey);

        BFTBridge bridge = BFTBridge(contractAddress);

        if (shouldPause) {
            bridge.pause();
            console.log('Contract paused');
        } else {
            bridge.unpause();
            console.log('Contract unpaused');
        }

        vm.stopBroadcast();
    }
}
