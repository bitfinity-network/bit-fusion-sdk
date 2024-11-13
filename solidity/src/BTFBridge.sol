// SPDX-License-Identifier: MIT
pragma solidity ^0.8.7;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "src/WrappedToken.sol";
import "src/interfaces/IFeeCharge.sol";
import { RingBuffer } from "src/libraries/RingBuffer.sol";
import "src/abstract/TokenManager.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";

contract BTFBridge is TokenManager, UUPSUpgradeable, OwnableUpgradeable, PausableUpgradeable {
    using RingBuffer for RingBuffer.RingBufferUint32;
    using SafeERC20 for IERC20;

    // Error codes:
    uint8 public constant MINT_ERROR_CODE_OK = 0;
    uint8 public constant MINT_ERROR_CODE_INSUFFICIENT_FEE_DEPOSIT = 1;
    uint8 public constant MINT_ERROR_CODE_ZERO_AMOUNT = 2;
    uint8 public constant MINT_ERROR_CODE_USED_NONCE = 3;
    uint8 public constant MINT_ERROR_CODE_ZERO_RECIPIENT = 4;
    uint8 public constant MINT_ERROR_CODE_UNEXPECTED_RECIPIENT_CHAIN_ID = 5;
    uint8 public constant MINT_ERROR_CODE_TOKENS_NOT_BRIDGED = 6;
    uint8 public constant MINT_ERROR_CODE_PROCESSING_NOT_REQUESTED = 7;

    // Gas fee for batch mint operation.
    uint256 constant COMMON_BATCH_MINT_GAS_FEE = 200000;

    // Gas fee for mint order processing.
    uint256 constant ORDER_BATCH_MINT_GAS_FEE = 100000;

    // Minimal amount of fee deposit to process mint order.
    uint256 constant MIN_FEE_DEPOSIT_AMOUNT = COMMON_BATCH_MINT_GAS_FEE + ORDER_BATCH_MINT_GAS_FEE;

    // Has a user's transaction nonce been used?
    mapping(bytes32 => mapping(uint32 => bool)) private _isNonceUsed;

    // Blocknumbers for users deposit Ids.
    mapping(address => mapping(uint8 => uint32)) private _userDepositBlocks;

    // Last 255 user's burn operations.
    mapping(address => RingBuffer.RingBufferUint32) private _lastUserBurns;

    // Address of feeCharge contract
    IFeeCharge public feeChargeContract;

    // Operation ID counter
    uint32 public operationIDCounter;

    // Address of minter canister
    address public minterCanisterAddress;

    /// Allowed implementations hash list
    mapping(bytes32 => bool) public allowedImplementations;

    /// Controller AccessList for adding implementations
    mapping(address => bool) public controllerAccessList;

    uint32 private constant MINT_ORDER_DATA_LEN = 269;

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
        uint32 recipientChainID;
        address approveSpender;
        uint256 approveAmount;
        address feePayer;
    }

    // Event for mint operation
    event MintTokenEvent(
        uint256 amount,
        bytes32 fromToken,
        bytes32 senderID,
        address toERC20,
        address recipient,
        uint32 nonce,
        uint256 chargedFee
    );

    /// Event for burn operation
    event BurnTokenEvent(
        address sender,
        uint256 amount,
        address fromERC20,
        bytes recipientID,
        bytes32 toToken,
        uint32 operationID,
        bytes32 name,
        bytes16 symbol,
        uint8 decimals,
        bytes32 memo
    );

    /// Event that can be emited with a notification for the minter canister
    event NotifyMinterEvent(uint32 notificationType, address txSender, bytes userData, bytes32 memo);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        // Locks the contract and prevent any future re-initialization
        _disableInitializers();
    }

    /// Initializes the BTFBridge contract.
    ///
    /// @param minterAddress The address of the minter canister.
    /// @param feeChargeAddress The address of the fee charge contract.
    /// @param isWrappedSide A boolean indicating whether this is the wrapped side of the bridge.
    /// @param owner The initial owner of the contract. If set to 0x0, the caller becomes the owner.
    /// @param controllers The initial list of authorized controllers.
    /// @dev This function is called only once during the contract deployment.
    function initialize(
        address minterAddress,
        address feeChargeAddress,
        address wrappedTokenDeployer,
        bool isWrappedSide,
        address owner,
        address[] memory controllers
    ) public initializer {
        minterCanisterAddress = minterAddress;
        feeChargeContract = IFeeCharge(feeChargeAddress);
        __TokenManager__init(isWrappedSide, wrappedTokenDeployer);

        // Set the owner
        address newOwner = owner != address(0) ? owner : msg.sender;
        __Ownable_init(newOwner);

        // Add owner to the controller list
        controllerAccessList[newOwner] = true;

        // Add controllers
        for (uint256 i = 0; i < controllers.length; i++) {
            controllerAccessList[controllers[i]] = true;
        }

        __UUPSUpgradeable_init();
        __Pausable_init();
    }

    /// Restrict who can upgrade this contract
    function _authorizeUpgrade(
        address newImplementation
    ) internal view override onlyOwner {
        require(allowedImplementations[newImplementation.codehash], "Not allowed implementation");
    }

    /// Pause the contract and prevent any future mint or burn operations
    /// Can be called only by the owner
    function pause() external onlyControllers {
        _pause();
    }

    /// Unpause the contract
    /// Can be called only by the owner
    function unpause() external onlyControllers {
        _unpause();
    }

    /// Modifier that restricts access to only addresses in the
    /// `controllerAccessList`.
    /// This modifier can be used on functions that should only be callable by authorized controllers.
    modifier onlyControllers() {
        require(controllerAccessList[msg.sender], "Not a controller");
        _;
    }

    /// Add a new implementation to the allowed list
    function addAllowedImplementation(
        bytes32 bytecodeHash
    ) external onlyControllers {
        require(!allowedImplementations[bytecodeHash], "Implementation already allowed");

        allowedImplementations[bytecodeHash] = true;
    }

    /// Emit minter notification event with the given `userData`. For details
    /// about what should be in the user data,
    /// check the implementation of the corresponding minter.
    function notifyMinter(uint32 notificationType, bytes calldata userData, bytes32 memo) external {
        emit NotifyMinterEvent(notificationType, msg.sender, userData, memo);
    }

    /// Adds the given `controller` address to the `controllerAccessList`.
    /// This function can only be called by the contract owner.
    function addController(
        address controller
    ) external onlyOwner {
        controllerAccessList[controller] = true;
    }

    /// Removes the given `controller` address from the `controllerAccessList`.
    /// This function can only be called by the contract owner.
    function removeController(
        address controller
    ) external onlyOwner {
        controllerAccessList[controller] = false;
    }

    /// Transfer funds to users according the signed encoded orders.
    /// Returns `processedOrders` array of error codes for each mint order;
    function batchMint(
        bytes calldata encodedOrders,
        bytes calldata signature,
        uint32[] calldata ordersToProcess
    ) external whenNotPaused returns (uint8[] memory) {
        require(encodedOrders.length > 0, "Expected non-empty orders batch");
        require(encodedOrders.length % MINT_ORDER_DATA_LEN == 0, "Incorrect mint orders batch encoding");
        _checkMinterSignature(encodedOrders, signature);

        uint32 ordersNumber = uint32(encodedOrders.length) / MINT_ORDER_DATA_LEN;

        bool[] memory orderIndexes = new bool[](ordersNumber);
        if (ordersToProcess.length == 0) {
            for (uint32 i = 0; i < ordersNumber; i++) {
                orderIndexes[i] = true;
            }
        } else {
            for (uint32 i = 0; i < ordersToProcess.length; i++) {
                uint32 orderIndex = ordersToProcess[i];
                orderIndexes[orderIndex] = true;
            }
        }

        uint8[] memory processedOrderIndexes = new uint8[](ordersNumber);
        uint32 processedOrdersNumber = 0;
        for (uint32 i = 0; i < ordersNumber; i++) {
            if (!orderIndexes[i]) {
                processedOrderIndexes[i] = MINT_ERROR_CODE_PROCESSING_NOT_REQUESTED;
                continue;
            }

            uint32 orderStart = MINT_ORDER_DATA_LEN * i;
            uint32 orderEnd = orderStart + MINT_ORDER_DATA_LEN;
            MintOrderData memory order = _decodeOrder(encodedOrders[orderStart:orderEnd]);

            // If user can't pay required fee, skip his order.
            if (_isFeeRequired()) {
                bool canPayFee = feeChargeContract.canPayFee(order.feePayer, order.senderID, MIN_FEE_DEPOSIT_AMOUNT);
                if (!canPayFee) {
                    processedOrderIndexes[i] = MINT_ERROR_CODE_INSUFFICIENT_FEE_DEPOSIT;
                    continue;
                }
            }

            /// If order is invalid, skip it.
            uint8 orderValidationResult = _isOrderValid(order);
            if (orderValidationResult != MINT_ERROR_CODE_OK) {
                processedOrderIndexes[i] = orderValidationResult;
                continue;
            }

            // Mint tokens according to the order.
            _mintInner(order);

            // Mark the order as processed.
            processedOrderIndexes[i] = MINT_ERROR_CODE_OK;
            processedOrdersNumber += 1;
        }

        // Charge fee for successfully processed orders and emit Minted event for each.
        uint256 feePerUser = 0;
        if (_isFeeRequired() && processedOrdersNumber > 0) {
            feePerUser = ((COMMON_BATCH_MINT_GAS_FEE / processedOrdersNumber) + ORDER_BATCH_MINT_GAS_FEE) * tx.gasprice;
        }

        for (uint32 i = 0; i < ordersNumber; i++) {
            if (processedOrderIndexes[i] == MINT_ERROR_CODE_OK) {
                // Array indexes inlined to soleve StackTooDeep problem.
                MintOrderData memory order =
                    _decodeOrder(encodedOrders[MINT_ORDER_DATA_LEN * i:MINT_ORDER_DATA_LEN * i + MINT_ORDER_DATA_LEN]);
                if (_isFeeRequired()) {
                    _chargeFee(order.feePayer, order.senderID, feePerUser);
                }
                _emitMintedEvent(order, feePerUser);
            }
        }

        return processedOrderIndexes;
    }

    function _mintInner(
        MintOrderData memory order
    ) private {
        // Update token's metadata only if it is a wrapped token
        bool isTokenWrapped = _wrappedToBase[order.toERC20] == order.fromTokenID;
        // the token must be registered or the side must be base
        require(isBaseSide() || isTokenWrapped, "Invalid token pair");

        if (isTokenWrapped) {
            updateTokenMetadata(order.toERC20, order.name, order.symbol, order.decimals);
        }

        // Execute the withdrawal
        _isNonceUsed[order.senderID][order.nonce] = true;
        IERC20(order.toERC20).safeTransfer(order.recipient, order.amount);

        if (order.approveSpender != address(0) && order.approveAmount != 0 && isTokenWrapped) {
            WrappedToken(order.toERC20).approveByOwner(order.recipient, order.approveSpender, order.approveAmount);
        }
    }

    /// Charge fee from the user.
    function _chargeFee(address from, bytes32 senderID, uint256 amount) private {
        if (amount != 0) {
            feeChargeContract.chargeFee(from, payable(minterCanisterAddress), senderID, amount);
        }
    }

    // Emit Minted event according to the order.
    function _emitMintedEvent(MintOrderData memory order, uint256 feeAmount) private {
        emit MintTokenEvent(
            order.amount, order.fromTokenID, order.senderID, order.toERC20, order.recipient, order.nonce, feeAmount
        );
    }

    /// Getter function for block numbers
    function getDepositBlocks() external view returns (uint32[] memory blockNumbers) {
        blockNumbers = _lastUserBurns[msg.sender].getAll();
    }

    /// Burn ERC 20 tokens there to make possible perform a mint on other side of the bridge.
    /// Caller should approve transfer in the given `from_erc20` token for the bridge contract.
    /// Returns operation ID if operation is succesfull.
    function burn(
        uint256 amount,
        address fromERC20,
        bytes32 toTokenID,
        bytes memory recipientID,
        bytes32 memo
    ) public whenNotPaused returns (uint32) {
        require(fromERC20 != address(this), "From address must not be BTF bridge address");
        require(fromERC20 != address(0), "Invalid from address; must not be zero address");
        // Check if the token is registered on the bridge or the side is base
        require(
            isBaseSide() || (_wrappedToBase[fromERC20] != bytes32(0) && _baseToWrapped[toTokenID] != address(0)),
            "Invalid from address; not registered in the bridge"
        );
        require(amount > 0, "Invalid burn amount");
        uint256 currentAllowance = IERC20(fromERC20).allowance(msg.sender, address(this));
        // Check if the user has enough allowance; on wrapped side, the bridge will approve the tokens by itself
        require(isWrappedSide || currentAllowance >= amount, "Insufficient allowance");

        // Authorize the bridge to transfer the tokens if the side is wrapped
        if (isWrappedSide && currentAllowance < amount) {
            WrappedToken(fromERC20).approveByOwner(msg.sender, address(this), amount);
        }

        IERC20(fromERC20).safeTransferFrom(msg.sender, address(this), amount);

        // Update user information about burn operations.
        _lastUserBurns[msg.sender].push(uint32(block.number));

        // get the token details
        TokenMetadata memory meta = getTokenMetadata(fromERC20);

        uint32 operationID = operationIDCounter++;

        emit BurnTokenEvent(
            msg.sender,
            amount,
            fromERC20,
            recipientID,
            toTokenID,
            operationID,
            meta.name,
            meta.symbol,
            meta.decimals,
            memo
        );

        return operationID;
    }

    /// Getter function for minter address
    function getMinterAddress() external view returns (address) {
        return minterCanisterAddress;
    }

    /// Returns true if mint fee must be charged.
    function _isFeeRequired() private view returns (bool) {
        return minterCanisterAddress == msg.sender && address(feeChargeContract) != address(0);
    }

    /// Function to validate the mint order.
    /// Reverts on failure.
    function _validateOrder(
        MintOrderData memory order
    ) private view {
        // Assert recipient address is not zero
        require(order.recipient != address(0), "Invalid destination address");

        // Check if amount is greater than zero
        require(order.amount > 0, "Invalid order amount");

        // Check if nonce is not stored in the list
        require(!_isNonceUsed[order.senderID][order.nonce], "Invalid nonce");

        // Check if withdrawal is happening on the correct chain
        require(block.chainid == order.recipientChainID, "Invalid chain ID");

        if (_wrappedToBase[order.toERC20] != bytes32(0)) {
            require(_baseToWrapped[order.fromTokenID] == order.toERC20, "SRC token and DST token must be a valid pair");
        }
    }

    /// Function to check if the mint order is valid.
    function _isOrderValid(
        MintOrderData memory order
    ) private view returns (uint8) {
        // Check recipient address is not zero
        if (order.recipient == address(0)) {
            return MINT_ERROR_CODE_ZERO_RECIPIENT;
        }

        // Check if amount is greater than zero
        if (order.amount == 0) {
            return MINT_ERROR_CODE_ZERO_AMOUNT;
        }

        // Check if nonce is not stored in the list
        if (_isNonceUsed[order.senderID][order.nonce]) {
            return MINT_ERROR_CODE_USED_NONCE;
        }

        // Check if withdrawal is happening on the correct chain
        if (block.chainid != order.recipientChainID) {
            return MINT_ERROR_CODE_UNEXPECTED_RECIPIENT_CHAIN_ID;
        }

        // Check if tokens are bridged.
        if (_wrappedToBase[order.toERC20] != bytes32(0) && _baseToWrapped[order.fromTokenID] != order.toERC20) {
            return MINT_ERROR_CODE_TOKENS_NOT_BRIDGED;
        }

        return MINT_ERROR_CODE_OK;
    }

    function _decodeOrderFeePayer(
        bytes calldata encodedOrder
    ) private pure returns (address) {
        return address(bytes20(encodedOrder[249:269]));
    }

    function _decodeOrderSenderID(
        bytes calldata encodedOrder
    ) private pure returns (bytes32) {
        return bytes32(encodedOrder[32:64]);
    }

    function _decodeOrder(
        bytes calldata encodedOrder
    ) private pure returns (MintOrderData memory order) {
        // Decode order data
        order.amount = uint256(bytes32(encodedOrder[:32]));
        order.senderID = _decodeOrderSenderID(encodedOrder);
        order.fromTokenID = bytes32(encodedOrder[64:96]);
        order.recipient = address(bytes20(encodedOrder[96:116]));
        order.toERC20 = address(bytes20(encodedOrder[116:136]));
        order.nonce = uint32(bytes4(encodedOrder[136:140]));
        order.senderChainID = uint32(bytes4(encodedOrder[140:144]));
        order.recipientChainID = uint32(bytes4(encodedOrder[144:148]));
        order.name = bytes32(encodedOrder[148:180]);
        order.symbol = bytes16(encodedOrder[180:196]);
        order.decimals = uint8(encodedOrder[196]);
        order.approveSpender = address(bytes20(encodedOrder[197:217]));
        order.approveAmount = uint256(bytes32(encodedOrder[217:249]));
        order.feePayer = _decodeOrderFeePayer(encodedOrder);
    }

    /// Function to check encodedOrder signature
    function _checkMintOrderSignature(
        bytes calldata encodedOrder
    ) private view {
        _checkMinterSignature(encodedOrder[:MINT_ORDER_DATA_LEN], encodedOrder[MINT_ORDER_DATA_LEN:]);
    }

    /// Function to check encodedOrder signature
    function _checkMinterSignature(bytes calldata data, bytes calldata signature) private view {
        // Create a hash of the order data
        bytes32 hash = keccak256(data);

        // Recover signer from the signature
        address signer = ECDSA.recover(hash, signature);

        // Check if signer is the minter canister
        require(signer == minterCanisterAddress, "Invalid signature");
    }
}
