// SPDX-License-Identifier: MIT
pragma solidity ^0.8.7;

import "openzeppelin-contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts/token/ERC20/utils/SafeERC20.sol";
import "src/WrappedToken.sol";

contract BFTBridge {
    using SafeERC20 for IERC20;

    struct MintOrderData {
        uint256 amount;
        bytes32 senderID;
        bytes32 fromTokenID;
        address recipient;
        address toERC20;
        uint32 nonce;
        bytes32 name;
        bytes16 symbol;
        uint8 decimals;
        uint32 senderChainID;
    }

    struct RingBuffer {
        uint8 begin;
        uint8 end;
    }

    function increment(
        RingBuffer memory buffer
    ) public pure returns (RingBuffer memory) {
        unchecked {
            buffer.end = buffer.end + 1;
        }
        if (buffer.begin == buffer.end) {
            unchecked {
                buffer.begin = buffer.begin + 1;
            }
        }
        return buffer;
    }

    // Function to get the size of the buffer
    function size(RingBuffer memory buffer) public pure returns (uint8) {
        uint8 sizeOf;
        if (buffer.begin <= buffer.end) {
            sizeOf = buffer.end - buffer.begin;
        } else {
            sizeOf = uint8(buffer.end + 256 - buffer.begin);
        }

        return sizeOf;
    }

    function truncateUTF8(
        string memory input
    ) public pure returns (bytes32 result) {
        // If the last byte starts with 0xxxxx, return the data as is
        bytes memory source = bytes(input);
        if (source.length < 32 || (source[31] & 0x80) == 0) {
            assembly {
                result := mload(add(source, 32))
            }
            return result;
        }

        if (source.length == 0) {
            return 0x0;
        }

        // Go backwards from the last byte until a byte that doesn't start with 10xxxxxx is found
        for (uint8 i = 31; i >= 0; i--) {
            if ((source[i] & 0xC0) != 0x80) {
                for (uint8 j = i; j < 32; j += 1) {
                    source[j] = 0;
                }

                assembly {
                    result := mload(add(source, 32))
                }

                break;
            }

            if (i == 0) {
                return 0x0;
            }
        }
    }

    function toIDfromBaseAddress(
        uint32 chainID,
        address toAddress
    ) public pure returns (bytes32 toID) {
        return
            bytes32(
                abi.encodePacked(
                    uint8(1),
                    chainID,
                    toAddress,
                    uint32(0),
                    uint16(0),
                    uint(8)
                )
            );
    }

    // Has a user's transaction nonce been used?
    mapping(bytes32 => mapping(uint32 => bool)) private _isNonceUsed;

    // Blocknumbers for users deposit Ids.
    mapping(address => mapping(uint8 => uint32)) private _userDepositBlocks;

    // Beginning and the end indices for the the user deposits
    mapping(address => RingBuffer) private _lastUserDeposit;

    // Get the wrapped token addresses given their native token.
    mapping(bytes32 => address) private _erc20TokenRegistry;

    // Mapping from Base tokens to Wrapped tokens
    mapping(address => bytes32) private _baseTokenRegistry;

    // Pending burn operations.
    // A burn operation should be finished with the `finishBurn` function.
    mapping(address => mapping(uint32 => Erc20BurnInfo)) private _pendingBurns;

    // Address of minter canister
    address public minterCanisterAddress;

    // Operataion ID counter
    uint32 public operationIDCounter;

    // Constructor to initialize minterCanisterAddress
    constructor(address minterAddress) {
        minterCanisterAddress = minterAddress;
    }

    // Event for mint operation
    event MintTokenEvent(
        uint256 amount,
        bytes32 fromToken,
        bytes32 senderID,
        address toERC20,
        address recipient,
        uint32 nonce
    );

    // Event for burn operation
    event BurnTokenEvent(
        address sender,
        uint256 amount,
        address fromERC20,
        bytes32 recipientID,
        bytes32 toToken,
        uint32 operationID,
        bytes32 name,
        bytes16 symbol,
        uint8 decimals
    );

    // Event for new wrapped token creation
    event WrappedTokenDeployedEvent(
        string name,
        string symbol,
        bytes32 baseTokenID,
        address wrappedERC20
    );

    // Struct with information about burn operation
    struct Erc20BurnInfo {
        address sender;
        uint256 amount;
        address fromERC20;
        bytes32 recipientID;
        bytes32 toToken;
        bytes32 name;
        bytes16 symbol;
        uint8 decimals;
    }

    // Main function to withdraw funds
    function mint(bytes calldata encodedOrder) external {
        MintOrderData memory order = _decodeAndValidateClaim(
            encodedOrder[:197]
        );

        _checkMintOrderSignature(encodedOrder);

        // Cases:
        // 1. `_erc20TokenRegistry` contains the `order.fromTokenID`. So, we are in WrappedToken side.
        // 2. `_erc20TokenRegistry` does not contain the `order.fromTokenID`. So:
        //   a. We are in BaseToken side.
        //   b. We are minting NativeToken.
        address toToken = _erc20TokenRegistry[order.fromTokenID];

        // If we mint base token we don't have information about token pairs.
        // So, we need to get it from the order.
        if (toToken == address(0)) {
            toToken = order.toERC20;
        }

        // The toToken should be a valid token address.
        // This should never fail.
        require(
            toToken != address(0),
            "toToken address should be specified correctly"
        );

        // Check the Token limits
        WrappedToken(toToken).setMetaData(
            order.name,
            order.symbol,
            order.decimals
        );

        // Execute the withdrawal
        _isNonceUsed[order.senderID][order.nonce] = true;
        IERC20(toToken).safeTransfer(order.recipient, order.amount);

        // Emit event
        emit MintTokenEvent(
            order.amount,
            order.fromTokenID,
            order.senderID,
            toToken,
            order.recipient,
            order.nonce
        );
    }

    // Getter function for block numbers
    function getDepositBlocks() external view returns (uint32[] memory) {
        RingBuffer memory buffer = _lastUserDeposit[msg.sender];

        // Get buffer size
        uint8 bufferSize = size(buffer);

        // Fill return buffer with values
        uint32[] memory res = new uint32[](bufferSize);
        for (uint8 i = buffer.begin; i != buffer.end; ) {
            res[i] = _userDepositBlocks[msg.sender][i]; // Assign values to the temporary array
            unchecked {
                i++;
            }
        }

        return res;
    }

    // Burn ERC 20 tokens there to make possible perform a mint on other side of the bridge.
    // Caller should approve transfer in the given `from_erc20` token for the bridge contract.
    // Returns operation ID if operation is succesfull.
    function burn(
        uint256 amount,
        address fromERC20,
        bytes32 recipientID
    ) public returns (uint32) {
        require(fromERC20 != address(this));

        IERC20(fromERC20).safeTransferFrom(msg.sender, address(this), amount);

        bytes32 toTokenID = _baseTokenRegistry[fromERC20];

        require(amount > 0, "Invalid burn amount");
        require(fromERC20 != address(0), "Invalid from address");
        require(recipientID != bytes32(0), "Invalid destination address");

        RingBuffer memory buffer = _lastUserDeposit[msg.sender];
        _userDepositBlocks[msg.sender][buffer.end] = uint32(block.number);
        _lastUserDeposit[msg.sender] = increment(buffer);

        // get the token details
        bytes32 name;
        bytes16 symbol;
        uint8 decimals;

        try IERC20Metadata(fromERC20).name() returns (string memory _name) {
            name = truncateUTF8(_name);
        } catch {}
        try IERC20Metadata(fromERC20).symbol() returns (
            string memory _symbol
        ) {
            symbol = bytes16(truncateUTF8(_symbol));
        } catch {}
        try IERC20Metadata(fromERC20).decimals() returns (uint8 _decimals) {
            decimals = _decimals;
        } catch {}

        uint32 operationID = operationIDCounter++;

        emit BurnTokenEvent(
            msg.sender,
            amount,
            fromERC20,
            recipientID,
            toTokenID,
            operationID,
            name,
            symbol,
            decimals
        );

        _pendingBurns[msg.sender][operationID] = Erc20BurnInfo(
            msg.sender,
            amount,
            fromERC20,
            recipientID,
            toTokenID,
            name,
            symbol,
            decimals
        );
        return operationID;
    }

    // Removes information about the burn operation from the contract.
    function finishBurn(uint32 operationID) public returns (bool) {
        // Burn operation with zero amount can't be done and never stored.
        if (_pendingBurns[msg.sender][operationID].amount != 0) {
            delete _pendingBurns[msg.sender][operationID];
            return true;
        }
        return false;
    }

    // Returns information about pending burn with the given operationID for the msg.sender.
    // If returned amount is zero, there is no pending burn.
    function getPendingBurnInfo(
        address user,
        uint32 operationID
    )
        public
        view
        returns (
            address sender,
            uint256 amount,
            address fromERC20,
            bytes32 recipientID,
            bytes32 toToken,
            bytes32 name,
            bytes16 symbol,
            uint8 decimals
        )
    {
        Erc20BurnInfo memory info = _pendingBurns[user][operationID];
        return (
            info.sender,
            info.amount,
            info.fromERC20,
            info.recipientID,
            info.toToken,
            info.name,
            info.symbol,
            info.decimals
        );
    }

    // Getter function for minter address
    function getMinterAddress() external view returns (address) {
        return minterCanisterAddress;
    }

    // Returns wrapped token for the given base token
    function getWrappedToken(
        bytes32 baseTokenID
    ) external view returns (address) {
        return _erc20TokenRegistry[baseTokenID];
    }
    
    // Returns base token for the given wrapped token
    function getBaseToken(
        address wrappedTokenAddress
    ) external view returns (bytes32) {
        return _baseTokenRegistry[wrappedTokenAddress];
    }

    // Creates a new ERC20 compatible token contract as a wrapper for the given `externalToken`.
    function deployERC20(
        string memory name,
        string memory symbol,
        bytes32 baseTokenID
    ) public returns (address) {
        require(
            _erc20TokenRegistry[baseTokenID] == address(0),
            "Wrapper already exist"
        );

        // Create the new token
        WrappedToken wrappedERC20 = new WrappedToken(
            name,
            symbol,
            address(this)
        );

        _erc20TokenRegistry[baseTokenID] = address(wrappedERC20);
        _baseTokenRegistry[address(wrappedERC20)] = baseTokenID;

        emit WrappedTokenDeployedEvent(
            name,
            symbol,
            baseTokenID,
            address(wrappedERC20)
        );

        return address(wrappedERC20);
    }

    // Function to decode and validate the order data
    function _decodeAndValidateClaim(
        bytes calldata encodedOrder
    ) private view returns (MintOrderData memory) {
        // Decode order data
        uint256 amount = uint256(bytes32(encodedOrder[:32]));
        bytes32 senderID = bytes32(encodedOrder[32:64]);
        bytes32 fromTokenID = bytes32(encodedOrder[64:96]);
        address recipient = address(bytes20(encodedOrder[96:116]));
        address toERC20 = address(bytes20(encodedOrder[116:136]));
        uint32 nonce = uint32(bytes4(encodedOrder[136:140]));
        uint32 senderChainId = uint32(bytes4(encodedOrder[140:144]));
        uint32 recipientChainId = uint32(bytes4(encodedOrder[144:148]));
        bytes32 name = bytes32(encodedOrder[148:180]);
        bytes16 symbol = bytes16(encodedOrder[180:196]);
        uint8 decimals = uint8(encodedOrder[196]);

        // Assert recipient address is not zero
        require(recipient != address(0), "Invalid destination address");

        // Check if amount is greater than zero
        require(amount > 0, "Invalid order amount");

        // Check if nonce is not stored in the list
        require(!_isNonceUsed[senderID][nonce], "Invalid nonce");

        // Check if withdrawal is happening on the correct chain
        require(block.chainid == recipientChainId, "Invalid chain ID");
        
        if (_baseTokenRegistry[toERC20] != bytes32(0)) {
            require(
                _erc20TokenRegistry[fromTokenID] == toERC20,
                "SRC token and DST token must be a valid pair"
            );
        }

        // Return the decoded order data
        return
            MintOrderData(
                amount,
                senderID,
                fromTokenID,
                recipient,
                toERC20,
                nonce,
                name,
                symbol,
                decimals,
                senderChainId
            );
    }

    // Function to check encodedOrder signature
    function _checkMintOrderSignature(
        bytes calldata encodedOrder
    ) private view {
        // Create a hash of the order data
        bytes32 hash = keccak256(encodedOrder[:197]);

        // Recover signer from the signature
        address signer = ECDSA.recover(hash, encodedOrder[197:]);

        // Check if signer is the minter canister
        require(signer == minterCanisterAddress, "Invalid signature");
    }
}
