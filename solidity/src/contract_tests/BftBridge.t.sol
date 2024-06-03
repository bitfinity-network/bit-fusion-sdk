// SPDX-License-Identifier: MIT
pragma solidity ^0.8.7;

import "forge-std/Test.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/utils/cryptography/ECDSA.sol";
import "src/BftBridge.sol";
import "src/WrappedToken.sol";

contract BftBridgeTest is Test {
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

    BFTBridge _bridge;

    function setUp() public {
        vm.chainId(_CHAIN_ID);
        _bridge = new BFTBridge(_owner);
    }

    function bytes32ToString(bytes32 _bytes32) public pure returns (string memory) {
        uint8 i = 0;
        while(i < 32 && _bytes32[i] != 0) {
            i++;
        }

        bytes memory bytesArray = new bytes(i);
        for (i = 0; i < 32 && _bytes32[i] != 0; i++) {
            bytesArray[i] = _bytes32[i];
        }

        return string(bytesArray);
    }

    function testTruncateUTF8() public {
        {
            bytes32 result = _bridge.truncateUTF8("");
            assertEq(bytes32ToString(result), "");
        }

        {
            bytes32 result = _bridge.truncateUTF8("1");
            assertEq(bytes32ToString(result), "1");
        }

        {
            bytes32 result = _bridge.truncateUTF8("123");
            assertEq(bytes32ToString(result), "123");
        }

        {
            bytes32 result = _bridge.truncateUTF8("12345678901234567890123456789012");
            assertEq(bytes32ToString(result), "12345678901234567890123456789012");
        }
        {
            bytes32 result = _bridge.truncateUTF8(unicode"1234567890123456789012345678901ї");
            assertEq(bytes32ToString(result), unicode"1234567890123456789012345678901");
        }

        {
            bytes32 result = _bridge.truncateUTF8(unicode"123456789012345678901234567890ї");
            assertEq(bytes32ToString(result), unicode"123456789012345678901234567890");
        }

        {
            bytes32 result = _bridge.truncateUTF8(unicode"123456789012345678ї");
            assertEq(bytes32ToString(result), unicode"123456789012345678ї");
        }

        {
            bytes32 result = _bridge.truncateUTF8(unicode"12345678901234567890її1");
            assertEq(bytes32ToString(result), unicode"12345678901234567890її1");
        }

        {
            bytes32 result = _bridge.truncateUTF8(unicode"ї");
            assertEq(bytes32ToString(result), unicode"ї");
        }
    }

    function testMinterCanisterAddress() public {
        assertEq(_bridge.minterCanisterAddress(), _owner);
    }

    function testMintERC20FromICRC2Success() public {
        MintOrder memory order = _createDefaultMintOrder();
        bytes memory encodedOrder = _encodeMintOrder(order, _OWNER_KEY);

        _bridge.mint(encodedOrder);

        assertEq(WrappedToken(order.toERC20).balanceOf(order.recipient), order.amount);
    }

    function testMintERC20FromICRC2InvalidChainID() public {
        MintOrder memory order = _createDefaultMintOrder();
        order.recipientChainID = 31000;

        bytes memory encodedOrder = _encodeMintOrder(order, _OWNER_KEY);

        vm.expectRevert(bytes("Invalid chain ID"));
        _bridge.mint(encodedOrder);
    }

    function testMintERC20FromICRC2InvalidRecipient() public {
        MintOrder memory order = _createDefaultMintOrder();
        order.recipient = address(0);

        bytes memory encodedOrder = _encodeMintOrder(order, _OWNER_KEY);
        vm.expectRevert(bytes("Invalid destination address"));
        _bridge.mint(encodedOrder);
    }

    function testMintERC20FromICRC2InvalidAmount() public {
        MintOrder memory order = _createDefaultMintOrder();
        order.amount = 0;

        bytes memory encodedOrder = _encodeMintOrder(order, _OWNER_KEY);

        vm.expectRevert(bytes("Invalid order amount"));
        _bridge.mint(encodedOrder);
    }

    function testMintERC20FromICRC2UsedNonce() public {
        MintOrder memory order = _createDefaultMintOrder();
        bytes memory encodedOrder = _encodeMintOrder(order, _OWNER_KEY);

        _bridge.mint(encodedOrder);

        order.amount = 200;
        order.recipient = _bob;
        encodedOrder = _encodeMintOrder(order, _OWNER_KEY);

        vm.expectRevert(bytes("Invalid nonce"));
        _bridge.mint(encodedOrder);
    }

    function testMintERC20FromICRC2InvalidPair() public {
        MintOrder memory order = _createDefaultMintOrder();
        order.fromTokenID = _createIdFromPrincipal(abi.encodePacked(uint8(1)));

        bytes memory encodedOrder = _encodeMintOrder(order, _OWNER_KEY);

        vm.expectRevert(bytes("SRC token and DST token must be a valid pair"));
        _bridge.mint(encodedOrder);
    }

    function testMintERC20FromICRC2InvalidSignature() public {
        MintOrder memory order = _createDefaultMintOrder();

        bytes memory encodedOrder = _encodeMintOrder(order, _OWNER_KEY);
        // make signature corrupted
        encodedOrder[0] = bytes1(uint8(42));

        vm.expectRevert(bytes("Invalid signature"));
        _bridge.mint(encodedOrder);
    }

    function testMintERC20FromICRC2InvalidOrderLength() public {
        bytes memory encodedOrder = abi.encodePacked(uint8(1), uint8(2), uint8(3), uint8(4));

        vm.expectRevert();
        _bridge.mint(encodedOrder);
    }

    function testGetWrappedToken() public {
        bytes32 base_token_id = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address wrapped_address = _bridge.deployERC20("Token", "TKN", base_token_id);
        assertEq(wrapped_address, _bridge.getWrappedToken(base_token_id));
    }

    function testGetBaseToken() public {
        bytes32 base_token_id = _createIdFromPrincipal(abi.encodePacked(uint8(1)));
        address wrapped_address = _bridge.deployERC20("Token", "TKN", base_token_id);
        assertEq(base_token_id, _bridge.getBaseToken(wrapped_address));
    }

    struct ExpectedBurnEvent {
        address sender;
        uint256 amount;
        address fromERC20;
        bytes32 recipientID;
        bytes32 toToken;
        bytes32 name;
        bytes16 symbol;
        uint8 decimals;
    }

    function _expectBurnEvent(ExpectedBurnEvent memory expected) private {
        Vm.Log[] memory entries = vm.getRecordedLogs();

        bool eventFound = false;

        for (uint i = 0; i < entries.length; i+=1) {
            if (entries[i].topics[0] == keccak256("BurnTokenEvent(address,uint256,address,bytes32,bytes32,bytes32,bytes16,uint8)")) {
                assertEq(eventFound, false);
                eventFound = true;

                assertEq(entries[i].emitter, address(_bridge));

                assertEq(entries[i].topics.length, 1);

                (address sender, uint256 amount, address fromERC20,
                    bytes32 recipientID, bytes32 toToken, bytes32 name,
                    bytes16 symbol, uint8 decimals) = abi.decode(entries[i].data,
                        (address, uint256, address, bytes32, bytes32, bytes32, bytes16, uint8));
                assertEq(expected.sender, sender);
                assertEq(expected.amount, amount);
                assertEq(expected.fromERC20, fromERC20);
                assertEq(expected.recipientID, recipientID);
                assertEq(expected.toToken, toToken);
                assertEq(expected.name, name);
                assertEq(expected.symbol, symbol);
                assertEq(expected.decimals, decimals);
            }
        }

        assertEq(eventFound, true);
    }

    function _createDefaultMintOrder() private returns (MintOrder memory order) {
        order.amount = 1000;
        order.senderID = _createIdFromPrincipal(abi.encodePacked(uint8(1), uint8(2), uint8(3)));
        order.fromTokenID = _createIdFromPrincipal(abi.encodePacked(uint8(1), uint8(2), uint8(3), uint8(4)));
        order.recipient = _alice;
        order.toERC20 = _bridge.deployERC20("Token", "TKN", order.fromTokenID);
        order.nonce = 0;
        order.senderChainID = 0;
        order.recipientChainID = _CHAIN_ID;
        order.name = _bridge.truncateUTF8("Token");
        order.symbol = bytes16(_bridge.truncateUTF8("Token"));
        order.decimals = 18;
        order.approveSpender = address(0);
        order.approveAmount = 0;
        order.feePayer = address(0);
    }

    function _encodeMintOrder(MintOrder memory order, uint256 privateKey) private pure returns (bytes memory) {
        // Encoding splitted in two parts to avoid problems with stack overflow.
        bytes memory encodedOrder = abi.encodePacked(order.amount, order.senderID, order.fromTokenID,
            order.recipient, order.toERC20, order.nonce, order.senderChainID, order.recipientChainID,
            order.name, order.symbol, order.decimals, order.approveSpender, order.approveAmount, address(0));
        // bytes memory encodedOrder = abi.encodePacked(partlyEncodedOrder, order.feePayer);
        bytes32 hash = keccak256(encodedOrder);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, hash);

        return abi.encodePacked(encodedOrder, r, s, v);
    }

    function _createIdFromPrincipal(bytes memory principal) private pure returns (bytes32) {
        return bytes32(abi.encodePacked(uint8(0), uint8(principal.length), principal));
    }

    function _createIdFromAddress(address addr, uint32 chainID) private pure returns (bytes32) {
        return bytes32(abi.encodePacked(uint8(1), chainID, addr));
    }
}
