// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import 'forge-std/Script.sol';
import 'forge-std/console.sol';
import "src/interfaces/IWrappedTokenDeployer.sol";

contract DeployWrappedToken is Script {
    function run() external {
        // Load environment variables or parameters
        address wrappedTokenDeployerAddress = vm.envAddress(
            'WRAPPED_TOKEN_DEPLOYER'
        );
        string memory name = vm.envString('NAME');
        string memory symbol = vm.envString('SYMBOL');
        uint8 decimals = uint8(vm.envUint('DECIMALS'));
        address owner = vm.envAddress('OWNER');

        // Validate inputs
        require(bytes(name).length > 0, 'Token name is required');
        require(bytes(symbol).length > 0, 'Token symbol is required');
        require(owner != address(0), 'Token owner is required');

        // Start broadcasting transactions
        vm.startBroadcast();

        // Get contract instance
        IWrappedTokenDeployer wrappedTokenDeployer = IWrappedTokenDeployer(
            wrappedTokenDeployerAddress
        );

        // Deploy ERC20 token
        address tokenAddress = wrappedTokenDeployer.deployERC20(
            name,
            symbol,
            decimals,
            owner
        );

        // Stop broadcasting transactions
        vm.stopBroadcast();

        // Log the deployed token address
        console.log('ERC20 deployed at:', tokenAddress);
    }
}
