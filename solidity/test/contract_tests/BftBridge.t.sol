// SPDX-License-Identifier: MIT
pragma solidity ^0.8.7;

import "forge-std/Test.sol";
import "forge-std/console.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "src/BftBridge.sol";
import "src/test_contracts/UUPSProxy.sol";
import "src/WrappedToken.sol";
import "src/WrappedTokenDeployer.sol";
import "src/libraries/StringUtils.sol";

contract BftBridgeTest is Test {
    using StringUtils for string;

    struct MintOrder {
        uint256 amount;
        bytes32 senderID;
        bytes32 fromTokenID;
        address recipient;
        address toERC20;
        uint32 nonce;
        uint32 senderChainID;
        uint32 recipientChainID;
        bytes32 name;
        bytes16 symbol;
        uint8 decimals;
        address approveSpender;
        uint256 approveAmount;
        address feePayer;
    }

    uint256 constant _OWNER_KEY = 1;
    uint256 constant _ALICE_KEY = 2;
    uint256 constant _BOB_KEY = 3;

    uint32 constant _CHAIN_ID = 31555;

    address _owner = vm.addr(_OWNER_KEY);
    address _alice = vm.addr(_ALICE_KEY);
    address _bob = vm.addr(_BOB_KEY);

    WrappedTokenDeployer _wrappedTokenDeployer;

    BFTBridge _wrappedBridge;
    BFTBridge _baseBridge;

    address newImplementation = address(8);

    address wrappedProxy;
    address baseProxy;

    function setUp() public {
        vm.chainId(_CHAIN_ID);
        vm.startPrank(_owner);

        _wrappedTokenDeployer = new WrappedTokenDeployer();

        // Encode the initialization call
        address[] memory initialControllers = new address[](0);

        // Encode the initialization call
        bytes memory initializeData = abi.encodeWithSelector(
            BFTBridge.initialize.selector,
            _owner,
            address(0),
            address(_wrappedTokenDeployer),
            true,
            _owner,
            initialControllers
        );

        BFTBridge wrappedImpl = new BFTBridge();

        UUPSProxy wrappedProxyContract = new UUPSProxy(address(wrappedImpl), initializeData);

        wrappedProxy = address(wrappedProxyContract);

        // Cast the proxy to BFTBridge
        _wrappedBridge = BFTBridge(address(wrappedProxy));

        // Encode the initialization call
        bytes memory baseInitializeData = abi.encodeWithSelector(
            BFTBridge.initialize.selector, _owner, address(0), _wrappedTokenDeployer, false, _owner, initialControllers
        );

        BFTBridge baseImpl = new BFTBridge();

        UUPSProxy baseProxyContract = new UUPSProxy(address(baseImpl), baseInitializeData);

        baseProxy = address(baseProxyContract);

        // Cast the proxy to BFTBridge
        _baseBridge = BFTBridge(address(baseProxy));

        vm.stopPrank();
    }

    function testMinterCanisterAddress() public view {
        assertEq(_wrappedBridge.minterCanisterAddress(), _owner);
    }

    // batch tests

    function testBatchMintSuccess() public {
        bytes32 base_token_id_1 = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address token1 = _wrappedBridge.deployERC20("WholaLottaLove", "LEDZEP", 21, base_token_id_1);
        MintOrder memory order_1 = _createDefaultMintOrder(base_token_id_1, token1, 0);

        bytes32 base_token_id_2 = _createIdFromPrincipal(abi.encodePacked(uint8(2)));
        address token2 = _wrappedBridge.deployERC20("Gabibbo", "GAB", 10, base_token_id_2);
        MintOrder memory order_2 = _createDefaultMintOrder(base_token_id_2, token2, 1);

        MintOrder[] memory orders = new MintOrder[](2);
        orders[0] = order_1;
        orders[1] = order_2;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](2);
        ordersToProcess[0] = 0;
        ordersToProcess[1] = 1;
        uint8[] memory processedOrders = _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        address recipient = order_1.recipient;
        uint256 amount = order_1.amount;

        assertEq(processedOrders[0], _wrappedBridge.MINT_ERROR_CODE_OK());
        assertEq(processedOrders[1], _wrappedBridge.MINT_ERROR_CODE_OK());

        assertEq(WrappedToken(token1).balanceOf(recipient), amount);
        assertEq(WrappedToken(token2).balanceOf(recipient), amount);
    }

    function testBatchMintProcessAllIfToProcessIsZero() public {
        bytes32 base_token_id_1 = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address token1 = _wrappedBridge.deployERC20("WholaLottaLove", "LEDZEP", 21, base_token_id_1);
        MintOrder memory order_1 = _createDefaultMintOrder(base_token_id_1, token1, 0);

        bytes32 base_token_id_2 = _createIdFromPrincipal(abi.encodePacked(uint8(2)));
        address token2 = _wrappedBridge.deployERC20("Gabibbo", "GAB", 10, base_token_id_2);
        MintOrder memory order_2 = _createDefaultMintOrder(base_token_id_2, token2, 1);

        MintOrder[] memory orders = new MintOrder[](2);
        orders[0] = order_1;
        orders[1] = order_2;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](0);
        uint8[] memory processedOrders = _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        address recipient = order_1.recipient;
        uint256 amount = order_1.amount;

        assertEq(processedOrders[0], _wrappedBridge.MINT_ERROR_CODE_OK());
        assertEq(processedOrders[1], _wrappedBridge.MINT_ERROR_CODE_OK());

        assertEq(WrappedToken(token1).balanceOf(recipient), amount);
        assertEq(WrappedToken(token2).balanceOf(recipient), amount);
    }

    function testBatchMintProcessOnlyIfRequested() public {
        bytes32 base_token_id_1 = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address token1 = _wrappedBridge.deployERC20("WholaLottaLove", "LEDZEP", 21, base_token_id_1);
        MintOrder memory order_1 = _createDefaultMintOrder(base_token_id_1, token1, 0);

        bytes32 base_token_id_2 = _createIdFromPrincipal(abi.encodePacked(uint8(2)));
        address token2 = _wrappedBridge.deployERC20("Gabibbo", "GAB", 10, base_token_id_2);
        MintOrder memory order_2 = _createDefaultMintOrder(base_token_id_2, token2, 1);

        MintOrder[] memory orders = new MintOrder[](2);
        orders[0] = order_1;
        orders[1] = order_2;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](1);
        ordersToProcess[0] = 0;
        uint8[] memory processedOrders = _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        address recipient = order_1.recipient;
        uint256 amount = order_1.amount;

        assertEq(processedOrders[0], _wrappedBridge.MINT_ERROR_CODE_OK());
        assertEq(processedOrders[1], _wrappedBridge.MINT_ERROR_CODE_PROCESSING_NOT_REQUESTED());

        assertEq(WrappedToken(token1).balanceOf(recipient), amount);
        assertEq(WrappedToken(token2).balanceOf(recipient), 0);
    }

    function testBatchMintInvalidChainID() public {
        MintOrder memory order = _createDefaultMintOrder();
        order.recipientChainID = 31000;

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](1);
        ordersToProcess[0] = 0;
        uint8[] memory processedOrders = _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        assertEq(processedOrders[0], _wrappedBridge.MINT_ERROR_CODE_UNEXPECTED_RECIPIENT_CHAIN_ID());

        assertEq(WrappedToken(order.toERC20).balanceOf(order.recipient), 0);
    }

    function testBatchMintInvalidRecipient() public {
        MintOrder memory order = _createDefaultMintOrder();
        order.recipient = address(0);

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](1);
        ordersToProcess[0] = 0;
        uint8[] memory processedOrders = _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        assertEq(processedOrders[0], _wrappedBridge.MINT_ERROR_CODE_ZERO_RECIPIENT());

        assertEq(WrappedToken(order.toERC20).balanceOf(order.recipient), 0);
    }

    function testBatchMintInvalidAmount() public {
        MintOrder memory order = _createDefaultMintOrder();
        order.amount = 0;

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](1);
        ordersToProcess[0] = 0;
        uint8[] memory processedOrders = _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        assertEq(processedOrders[0], _wrappedBridge.MINT_ERROR_CODE_ZERO_AMOUNT());

        assertEq(WrappedToken(order.toERC20).balanceOf(order.recipient), 0);
    }

    function testBatchMintUsedNonce() public {
        bytes32 base_token_id_1 = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address token1 = _wrappedBridge.deployERC20("WholaLottaLove", "LEDZEP", 21, base_token_id_1);
        MintOrder memory order_1 = _createDefaultMintOrder(base_token_id_1, token1, 0);

        bytes32 base_token_id_2 = _createIdFromPrincipal(abi.encodePacked(uint8(2)));
        address token2 = _wrappedBridge.deployERC20("Gabibbo", "GAB", 10, base_token_id_2);
        MintOrder memory order_2 = _createDefaultMintOrder(base_token_id_2, token2, 0);

        MintOrder[] memory orders = new MintOrder[](2);
        orders[0] = order_1;
        orders[1] = order_2;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](2);
        ordersToProcess[0] = 0;
        ordersToProcess[1] = 1;
        uint8[] memory processedOrders = _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        assertEq(processedOrders[0], _wrappedBridge.MINT_ERROR_CODE_OK());
        assertEq(processedOrders[1], _wrappedBridge.MINT_ERROR_CODE_USED_NONCE());

        assertEq(WrappedToken(order_2.toERC20).balanceOf(order_2.recipient), 0);
    }

    function testBatchMintInvalidPair() public {
        MintOrder memory order = _createDefaultMintOrder();
        order.fromTokenID = _createIdFromPrincipal(abi.encodePacked(uint8(1)));

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](1);
        ordersToProcess[0] = 0;

        uint8[] memory processedOrders = _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        assertEq(processedOrders[0], _wrappedBridge.MINT_ERROR_CODE_TOKENS_NOT_BRIDGED());

        assertEq(WrappedToken(order.toERC20).balanceOf(order.recipient), 0);
    }

    function testBatchMintInvalidSignature() public {
        bytes32 base_token_id_1 = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address token1 = _wrappedBridge.deployERC20("WholaLottaLove", "LEDZEP", 21, base_token_id_1);
        MintOrder memory order_1 = _createDefaultMintOrder(base_token_id_1, token1, 0);

        bytes32 base_token_id_2 = _createIdFromPrincipal(abi.encodePacked(uint8(2)));
        address token2 = _wrappedBridge.deployERC20("Gabibbo", "GAB", 10, base_token_id_2);
        MintOrder memory order_2 = _createDefaultMintOrder(base_token_id_2, token2, 1);

        MintOrder[] memory orders = new MintOrder[](2);
        orders[0] = order_1;
        orders[1] = order_2;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = new bytes(0);

        uint32[] memory ordersToProcess = new uint32[](2);
        ordersToProcess[0] = 0;
        ordersToProcess[1] = 1;

        vm.expectRevert();
        _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);
    }

    function testBatchMintInvalidOrderLength() public {
        bytes memory badEncodedOrder = abi.encodePacked(uint8(1), uint8(2), uint8(3), uint8(4));

        bytes32 base_token_id_1 = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address token1 = _wrappedBridge.deployERC20("WholaLottaLove", "LEDZEP", 21, base_token_id_1);
        MintOrder memory order_1 = _createDefaultMintOrder(base_token_id_1, token1, 0);

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order_1;
        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](1);
        ordersToProcess[0] = 0;

        vm.expectRevert();
        _wrappedBridge.batchMint(badEncodedOrder, signature, ordersToProcess);
    }

    function testGetWrappedToken() public {
        bytes32 base_token_id = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address wrapped_address = _wrappedBridge.deployERC20("Token", "TKN", 18, base_token_id);
        assertEq(wrapped_address, _wrappedBridge.getWrappedToken(base_token_id));
    }

    function testGetBaseToken() public {
        bytes32 base_token_id = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address wrapped_address = _wrappedBridge.deployERC20("Token", "TKN", 18, base_token_id);
        assertEq(base_token_id, _wrappedBridge.getBaseToken(wrapped_address));
    }

    // Creates a wrapped token with custom name, symbol, and decimals
    function testDeployERC20CustomDecimals() public {
        bytes32 base_token_id = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address wrapped_address = _wrappedBridge.deployERC20("WholaLottaLove", "LEDZEP", 21, base_token_id);
        WrappedToken token = WrappedToken(wrapped_address);
        assertEq(token.name(), "WholaLottaLove");
        assertEq(token.symbol(), "LEDZEP");
        assertEq(token.decimals(), 21);
    }

    function testListTokenPairs() public {
        bytes32[3] memory base_token_ids = [
            _createIdFromPrincipal(abi.encodePacked(uint8(1))),
            _createIdFromPrincipal(abi.encodePacked(uint8(2))),
            _createIdFromPrincipal(abi.encodePacked(uint8(3)))
        ];

        address[3] memory wrapped_tokens;
        for (uint256 i = 0; i < 3; i++) {
            address wrapped_address = _wrappedBridge.deployERC20("Token", "TKN", 18, base_token_ids[i]);
            wrapped_tokens[i] = wrapped_address;
        }

        (address[] memory wrapped, bytes32[] memory base) = _wrappedBridge.listTokenPairs();

        for (uint256 i = 0; i < 3; i++) {
            assertEq(wrapped[i], wrapped_tokens[i]);
            assertEq(base[i], base_token_ids[i]);
        }
    }

    function testBurnWrappedSideWithoutApprove() public {
        bytes memory principal = abi.encodePacked(uint8(1), uint8(2), uint8(3));

        // deploy erc20 so it can be used
        MintOrder memory order = _createSelfMintOrder();

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order;

        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](0);
        _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        assertEq(WrappedToken(order.toERC20).balanceOf(address(_owner)), order.amount);

        bytes32 memo = bytes32(abi.encodePacked(uint8(0)));

        vm.prank(address(_owner));
        _wrappedBridge.burn(1, order.toERC20, order.fromTokenID, principal, memo);
    }

    function testBurnBaseSideWithoutApproveShouldFail() public {
        bytes memory principal = abi.encodePacked(uint8(1), uint8(2), uint8(3));

        WrappedToken erc20 = new WrappedToken("omar", "OMAR", 18, _owner);
        address erc20Address = address(erc20);

        vm.prank(address(_owner));
        erc20.transfer(address(_owner), 100);

        bytes32 toTokenId = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        vm.prank(address(_owner));
        vm.expectRevert(bytes("Insufficient allowance"));

        bytes32 memo = bytes32(abi.encodePacked(uint8(0)));
        _baseBridge.burn(100, erc20Address, toTokenId, principal, memo);
    }

    function testBurnWrappedSideWithDeployedErc20() public {
        bytes memory principal = abi.encodePacked(uint8(1), uint8(2), uint8(3));

        // deploy erc20 so it can be used
        MintOrder memory order = _createSelfMintOrder();

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order;

        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](0);

        vm.prank(address(_owner));
        IERC20(order.toERC20).approve(address(_wrappedBridge), 1000);

        _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        assertEq(WrappedToken(order.toERC20).balanceOf(address(_owner)), order.amount);

        vm.prank(address(_owner));
        bytes32 memo = bytes32(abi.encodePacked(uint8(0)));
        _wrappedBridge.burn(1, order.toERC20, order.fromTokenID, principal, memo);
    }

    function testBurnWrappedSideWithUnregisteredToken() public {
        bytes memory principal = abi.encodePacked(uint8(1), uint8(2), uint8(3));

        address erc20 = address(new WrappedToken("omar", "OMAR", 18, _owner));

        bytes32 toTokenId = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        vm.expectRevert(bytes("Invalid from address; not registered in the bridge"));
        bytes32 memo = bytes32(abi.encodePacked(uint8(0)));
        _wrappedBridge.burn(100, erc20, toTokenId, principal, memo);
    }

    function testBurnBaseSideWithUnregisteredToken() public {
        bytes memory principal = abi.encodePacked(uint8(1), uint8(2), uint8(3));

        WrappedToken erc20 = new WrappedToken("omar", "OMAR", 18, _owner);
        address erc20Address = address(erc20);

        vm.prank(address(_owner));
        erc20.transfer(address(_owner), 100);
        vm.prank(address(_owner));
        erc20.approve(address(_baseBridge), 100);

        bytes32 toTokenId = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        vm.prank(address(_owner));
        bytes32 memo = bytes32(abi.encodePacked(uint8(0)));
        _baseBridge.burn(100, erc20Address, toTokenId, principal, memo);
    }

    function testMintBaseSideWithUnregisteredToken() public {
        WrappedToken erc20 = new WrappedToken("omar", "OMAR", 18, _owner);
        address erc20Address = address(erc20);

        vm.prank(address(_owner));
        erc20.transfer(address(_baseBridge), 1000);

        MintOrder memory order = _createMintOrder(_alice, erc20Address);

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order;

        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](0);

        _baseBridge.batchMint(encodedOrders, signature, ordersToProcess);

        assertEq(erc20.balanceOf(order.recipient), order.amount);
    }

    function testMintWrappedSideWithUnregisteredToken() public {
        WrappedToken erc20 = new WrappedToken("omar", "OMAR", 18, _owner);
        address erc20Address = address(erc20);

        vm.prank(address(_owner));
        erc20.transfer(address(_wrappedBridge), 1000);

        MintOrder memory order = _createMintOrder(_alice, erc20Address);

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = order;

        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](0);

        vm.expectRevert(bytes("Invalid token pair"));
        _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);
    }

    function testMintCallsAreRejectedWhenPaused() public {
        vm.prank(_owner);

        _wrappedBridge.pause();

        MintOrder memory mintOrder = _createDefaultMintOrder();

        MintOrder[] memory orders = new MintOrder[](1);
        orders[0] = mintOrder;

        bytes memory encodedOrders = _batchMintOrders(orders);
        bytes memory signature = _batchMintOrdersSignature(encodedOrders, _OWNER_KEY);

        uint32[] memory ordersToProcess = new uint32[](0);

        vm.expectRevert(abi.encodeWithSignature("EnforcedPause()"));
        _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);

        vm.prank(_owner);
        _wrappedBridge.unpause();

        // mint will be success
        _wrappedBridge.batchMint(encodedOrders, signature, ordersToProcess);
    }

    function testAddAllowedImplementation() public {
        vm.startPrank(_owner, _owner);

        BFTBridge _newImpl = new BFTBridge();

        newImplementation = address(_newImpl);

        _wrappedBridge.addAllowedImplementation(newImplementation.codehash);

        assertTrue(_wrappedBridge.allowedImplementations(newImplementation.codehash));

        vm.stopPrank();
    }

    function testAddAllowedImplementationOnlyOwner() public {
        vm.prank(address(10));

        vm.expectRevert();

        _wrappedBridge.addAllowedImplementation(newImplementation.codehash);
    }

    function testAddAllowedImplementationByAController() public {
        vm.startPrank(_owner);
        BFTBridge _newImpl = new BFTBridge();

        newImplementation = address(_newImpl);

        address controller = address(55);
        _wrappedBridge.addController(controller);

        vm.stopPrank();

        vm.prank(controller);

        _wrappedBridge.addAllowedImplementation(newImplementation.codehash);
    }

    /// Test that the bridge can be upgraded to a new implementation
    /// and the new implementation has been added to the list of allowed
    /// implementations
    function testUpgradeBridgeWithAllowedImplementation() public {
        vm.startPrank(_owner);

        BFTBridge _newImpl = new BFTBridge();

        newImplementation = address(_newImpl);

        _wrappedBridge.addAllowedImplementation(newImplementation.codehash);
        assertTrue(_wrappedBridge.allowedImplementations(newImplementation.codehash));

        // Wrap in ABI for easier testing
        BFTBridge proxy = BFTBridge(wrappedProxy);

        // pass empty calldata to initialize
        bytes memory data = new bytes(0);

        proxy.upgradeToAndCall(address(_newImpl), data);

        vm.stopPrank();
    }

    function testUpgradeBridgeWithNotAllowedImplementation() public {
        vm.startPrank(_owner);
        BFTBridge _newImpl = new BFTBridge();
        newImplementation = address(_newImpl);
        // Wrap in ABI for easier testing

        BFTBridge proxy = BFTBridge(wrappedProxy);
        // pass empty calldata to initialize
        bytes memory data = new bytes(0);
        vm.expectRevert();
        proxy.upgradeToAndCall(address(_newImpl), data);

        vm.stopPrank();
    }

    function testPauseByController() public {
        address controller = address(42);

        vm.prank(_owner);
        _wrappedBridge.addController(controller);

        vm.prank(controller);
        _wrappedBridge.pause();

        assertTrue(_wrappedBridge.paused());
    }

    function testPauseByNonController() public {
        address nonController = address(43);

        vm.prank(nonController);
        vm.expectRevert("Not a controller");
        _wrappedBridge.pause();
    }

    function testUnpauseByController() public {
        address controller = address(42);

        vm.prank(_owner);
        _wrappedBridge.addController(controller);

        vm.prank(controller);
        _wrappedBridge.pause();

        vm.prank(controller);
        _wrappedBridge.unpause();

        assertFalse(_wrappedBridge.paused());
    }

    function testUnpauseByNonController() public {
        address nonController = address(43);

        vm.prank(_owner);
        _wrappedBridge.pause();

        vm.prank(nonController);
        vm.expectRevert("Not a controller");
        _wrappedBridge.unpause();
    }

    function testAddAllowedImplementationByController() public {
        address controller = address(42);

        vm.prank(_owner);
        _wrappedBridge.addController(controller);

        bytes32 newImplementationHash = keccak256(abi.encodePacked("new implementation"));

        vm.prank(controller);
        _wrappedBridge.addAllowedImplementation(newImplementationHash);

        assertTrue(_wrappedBridge.allowedImplementations(newImplementationHash));
    }

    function testAddAllowedImplementationByNonController() public {
        address nonController = address(43);
        bytes32 newImplementationHash = keccak256(abi.encodePacked("new implementation"));

        vm.prank(nonController);
        vm.expectRevert("Not a controller");
        _wrappedBridge.addAllowedImplementation(newImplementationHash);
    }

    function testAddAllowedImplementationAlreadyAllowed() public {
        address controller = address(42);

        vm.prank(_owner);
        _wrappedBridge.addController(controller);

        bytes32 newImplementationHash = keccak256(abi.encodePacked("new implementation"));

        vm.prank(controller);
        _wrappedBridge.addAllowedImplementation(newImplementationHash);

        vm.prank(controller);
        vm.expectRevert("Implementation already allowed");
        _wrappedBridge.addAllowedImplementation(newImplementationHash);
    }

    function testAddAndRemoveController() public {
        address newController = address(44);

        vm.prank(_owner);
        _wrappedBridge.addController(newController);
        assertTrue(_wrappedBridge.controllerAccessList(newController));

        vm.prank(_owner);
        _wrappedBridge.removeController(newController);
        assertFalse(_wrappedBridge.controllerAccessList(newController));
    }

    function _createDefaultMintOrder() private returns (MintOrder memory order) {
        return _createDefaultMintOrder(0);
    }

    function _createDefaultMintOrder(
        uint32 nonce
    ) private returns (MintOrder memory order) {
        bytes32 fromTokenId = _createIdFromPrincipal(abi.encodePacked(uint8(1), uint8(2), uint8(3), uint8(4)));
        address toErc20 = _wrappedBridge.deployERC20("Token", "TKN", 18, fromTokenId);

        return _createDefaultMintOrder(fromTokenId, toErc20, nonce);
    }

    function _createDefaultMintOrder(
        bytes32 fromTokenId,
        address toERC20,
        uint32 nonce
    ) private view returns (MintOrder memory order) {
        order.amount = 1000;
        order.senderID = _createIdFromPrincipal(abi.encodePacked(uint8(1), uint8(2), uint8(3)));
        order.fromTokenID = fromTokenId;
        order.recipient = _alice;
        order.toERC20 = toERC20;
        order.nonce = nonce;
        order.senderChainID = 0;
        order.recipientChainID = _CHAIN_ID;
        order.name = StringUtils.truncateUTF8("Token");
        order.symbol = bytes16(StringUtils.truncateUTF8("Token"));
        order.decimals = 18;
        order.approveSpender = address(0);
        order.approveAmount = 0;
        order.feePayer = address(0);
    }

    function _createSelfMintOrder() private returns (MintOrder memory order) {
        order.amount = 1000;
        order.senderID = _createIdFromPrincipal(abi.encodePacked(uint8(1), uint8(2), uint8(3)));
        order.fromTokenID = _createIdFromPrincipal(abi.encodePacked(uint8(1), uint8(2), uint8(3), uint8(4)));
        order.recipient = address(_owner);
        order.toERC20 = _wrappedBridge.deployERC20("Token", "TKN", 18, order.fromTokenID);
        order.nonce = 0;
        order.senderChainID = 0;
        order.recipientChainID = _CHAIN_ID;
        // order.name = _bridge.truncateUTF8("Token");
        order.name = StringUtils.truncateUTF8("Token");
        // order.symbol = bytes16(_bridge.truncateUTF8("Token"));
        order.symbol = bytes16(StringUtils.truncateUTF8("Token"));
        order.decimals = 18;
        order.approveSpender = address(0);
        order.approveAmount = 0;
        order.feePayer = address(0);
    }

    function _createMintOrder(address recipient, address toERC20) private pure returns (MintOrder memory order) {
        order.amount = 1000;
        order.senderID = _createIdFromPrincipal(abi.encodePacked(uint8(1), uint8(2), uint8(3)));
        order.fromTokenID = _createIdFromPrincipal(abi.encodePacked(uint8(1), uint8(2), uint8(3), uint8(4)));
        order.recipient = recipient;
        order.toERC20 = toERC20;
        order.nonce = 0;
        order.senderChainID = 0;
        order.recipientChainID = _CHAIN_ID;
        // order.name = _bridge.truncateUTF8("Token");
        order.name = StringUtils.truncateUTF8("Token");
        // order.symbol = bytes16(_bridge.truncateUTF8("Token"));
        order.symbol = bytes16(StringUtils.truncateUTF8("Token"));
        order.decimals = 18;
        order.approveSpender = address(0);
        order.approveAmount = 0;
        order.feePayer = address(0);
    }

    function _batchMintOrders(
        MintOrder[] memory orders
    ) private pure returns (bytes memory) {
        bytes memory encodedOrders;
        for (uint256 i = 0; i < orders.length; i += 1) {
            bytes memory orderData = _encodeOrder(orders[i]);
            encodedOrders = abi.encodePacked(encodedOrders, orderData);
        }

        return abi.encodePacked(encodedOrders);
    }

    function _encodeOrder(
        MintOrder memory order
    ) private pure returns (bytes memory) {
        return abi.encodePacked(
            order.amount,
            order.senderID,
            order.fromTokenID,
            order.recipient,
            order.toERC20,
            order.nonce,
            order.senderChainID,
            order.recipientChainID,
            order.name,
            order.symbol,
            order.decimals,
            order.approveSpender,
            order.approveAmount,
            address(0)
        );
    }

    function _batchMintOrdersSignature(
        bytes memory encodedOrders,
        uint256 privateKey
    ) private pure returns (bytes memory) {
        bytes32 hash = keccak256(encodedOrders);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, hash);

        return abi.encodePacked(r, s, v);
    }

    function _createIdFromPrincipal(
        bytes memory principal
    ) private pure returns (bytes32) {
        return bytes32(abi.encodePacked(uint8(0), uint8(principal.length), principal));
    }

    function _createIdFromAddress(address addr, uint32 chainID) private pure returns (bytes32) {
        return bytes32(abi.encodePacked(uint8(1), chainID, addr));
    }
}
