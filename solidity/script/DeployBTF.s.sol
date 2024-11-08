// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "forge-std/Vm.sol";
import "@openzeppelin/foundry-upgrades/Upgrades.sol";
import { BTFBridge } from "src/BTFBridge.sol";

abstract contract DeployScript is Script {
    uint256 public immutable privateKey;
    bytes public data;
    address public proxyAddress;

    // contract name
    string public contractName = "BTFBridge.sol:BTFBridge";

    error InvalidAddress(string reason);

    modifier create() {
        _;
        proxyAddress = address(Upgrades.deployUUPSProxy(contractName, data));
    }

    constructor(
        uint256 pkey
    ) {
        privateKey = pkey;
    }

    function run() external {
        vm.startBroadcast(privateKey);
        _run();
        console.log("Proxy address: %s", proxyAddress);
        console.log("Implementation address: %s", getImplementation());
        vm.stopBroadcast();
    }

    function _run() internal virtual;

    function getImplementation() public view returns (address) {
        return Upgrades.getImplementationAddress(proxyAddress);
    }
}

contract DeploBTFTBridge is DeployScript {
    constructor() DeployScript(vm.envUint("PRIVATE_KEY")) { }

    address minterAddress = vm.envAddress("MINTER_ADDRESS");
    address feeChargeAddress = vm.envAddress("FEE_CHARGE_ADDRESS");
    address wrappedTokenDeployer = vm.envAddress("WRAPPED_TOKEN_DEPLOYER");
    bool isWrappedSide = vm.envBool("IS_WRAPPED_SIDE");
    address addressZero = address(0);
    address owner = vm.envOr("OWNER", addressZero);
    address[] zeroAddressControllers = new address[](0);
    address[] controllers = vm.envOr("CONTROLLERS", ",", zeroAddressControllers);

    function _run() internal override create {
        data = abi.encodeWithSelector(
            BTFBridge.initialize.selector,
            minterAddress,
            feeChargeAddress,
            wrappedTokenDeployer,
            isWrappedSide,
            owner,
            controllers
        );
    }
}
