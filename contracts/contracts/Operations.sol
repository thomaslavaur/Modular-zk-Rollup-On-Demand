// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "./Bytes.sol";
import "./Utils.sol";

/// @title zkSync operations tools
library Operations {
    // Circuit ops and their pubdata (chunks * bytes)

    /// @notice zkSync circuit operation type
    enum OpType {
        Noop,
        Deposit,
        TransferToNew,
        PartialExit,
        _CloseAccount, // used for correct op id offset
        Transfer,
        FullExit,
        ChangePubKey,
        ForcedExit,
        MintNFT,
        WithdrawNFT,
        Swap,
        ChangeGroup,
        FullChangeGroup
    }

    // Byte lengths

    uint8 internal constant OP_TYPE_BYTES = 1;
    uint8 internal constant TOKEN_BYTES = 4;
    uint8 internal constant PUBKEY_BYTES = 32;
    uint8 internal constant NONCE_BYTES = 4;
    uint8 internal constant PUBKEY_HASH_BYTES = 20;
    uint8 internal constant ADDRESS_BYTES = 20;
    uint8 internal constant GROUP_BYTES = 2;
    uint8 internal constant CONTENT_HASH_BYTES = 32;
    /// @dev Packed fee bytes lengths
    uint8 internal constant FEE_BYTES = 2;
    /// @dev zkSync account id bytes lengths
    uint8 internal constant ACCOUNT_ID_BYTES = 4;
    /// @dev zkSync nft serial id bytes lengths
    uint8 internal constant NFT_SERIAL_ID_BYTES = 4;
    uint8 internal constant AMOUNT_BYTES = 16;
    /// @dev Signature (for example full exit signature) bytes length
    uint8 internal constant SIGNATURE_BYTES = 64;

    // Deposit pubdata
    struct Deposit {
        // uint8 opType
        uint32 accountId; // 4B
        uint32 tokenId; // 4B
        uint128 amount; // 16B
        address owner; // 20B
        uint16 group; // 2B
    }

    uint256 internal constant PACKED_DEPOSIT_PUBDATA_BYTES =
        OP_TYPE_BYTES + ACCOUNT_ID_BYTES + TOKEN_BYTES + AMOUNT_BYTES + ADDRESS_BYTES + GROUP_BYTES;

    /// Deserialize deposit pubdata
    function readDepositPubdata(bytes memory _data) internal pure returns (Deposit memory parsed) {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint256 offset = OP_TYPE_BYTES;
        (offset, parsed.accountId) = Bytes.readUInt32(_data, offset); // accountId
        (offset, parsed.tokenId) = Bytes.readUInt32(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        (offset, parsed.owner) = Bytes.readAddress(_data, offset); // owner
        (offset, parsed.group) = Bytes.readUInt16(_data, offset); // group
        require(offset == PACKED_DEPOSIT_PUBDATA_BYTES, "N"); // reading invalid deposit pubdata size
    }

    /// Serialize deposit pubdata
    function writeDepositPubdataForPriorityQueue(Deposit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            uint8(OpType.Deposit),
            bytes4(0), // accountId (ignored) (update when ACCOUNT_ID_BYTES is changed)
            op.tokenId, // tokenId
            op.amount, // amount
            op.owner, // owner
            op.group // group
        );
    }

    /// @notice Write deposit pubdata for priority queue check.
    function checkDepositInPriorityQueue(Deposit memory op, bytes20 hashedPubdata) internal pure returns (bool) {
        return Utils.hashBytesToBytes20(writeDepositPubdataForPriorityQueue(op)) == hashedPubdata;
    }

    // FullExit pubdata

    struct FullExit {
        // uint8 opType
        uint32 accountId;
        address owner;
        uint32 tokenId;
        uint16 group;
        uint128 amount;
        uint32 nftCreatorAccountId;
        address nftCreatorAddress;
        uint32 nftSerialId;
        bytes32 nftContentHash;
    }

    uint256 public constant PACKED_FULL_EXIT_PUBDATA_BYTES =
        OP_TYPE_BYTES +
            ACCOUNT_ID_BYTES +
            ADDRESS_BYTES +
            TOKEN_BYTES +
            AMOUNT_BYTES +
            ACCOUNT_ID_BYTES +
            ADDRESS_BYTES +
            NFT_SERIAL_ID_BYTES +
            CONTENT_HASH_BYTES +
            GROUP_BYTES;

    function readFullExitPubdata(bytes memory _data) internal pure returns (FullExit memory parsed) {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint256 offset = OP_TYPE_BYTES;
        (offset, parsed.accountId) = Bytes.readUInt32(_data, offset); // accountId
        (offset, parsed.owner) = Bytes.readAddress(_data, offset); // owner
        (offset, parsed.tokenId) = Bytes.readUInt32(_data, offset); // tokenId
        (offset, parsed.group) = Bytes.readUInt16(_data, offset); // group
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        (offset, parsed.nftCreatorAccountId) = Bytes.readUInt32(_data, offset); // nftCreatorAccountId
        (offset, parsed.nftCreatorAddress) = Bytes.readAddress(_data, offset); // nftCreatorAddress
        (offset, parsed.nftSerialId) = Bytes.readUInt32(_data, offset); // nftSerialId
        (offset, parsed.nftContentHash) = Bytes.readBytes32(_data, offset); // nftContentHash
        require(offset == PACKED_FULL_EXIT_PUBDATA_BYTES, "O"); // reading invalid full exit pubdata size
    }

    function writeFullExitPubdataForPriorityQueue(FullExit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            uint8(OpType.FullExit),
            op.accountId, // accountId
            op.owner, // owner
            op.tokenId, // tokenId
            op.group, // group
            uint128(0), // amount -- ignored
            uint32(0), // nftCreatorAccountId -- ignored
            address(0), // nftCreatorAddress -- ignored
            uint32(0), // nftSerialId -- ignored
            bytes32(0) // nftContentHash -- ignored
        );
    }

    function checkFullExitInPriorityQueue(FullExit memory op, bytes20 hashedPubdata) internal pure returns (bool) {
        return Utils.hashBytesToBytes20(writeFullExitPubdataForPriorityQueue(op)) == hashedPubdata;
    }

    // PartialExit pubdata

    struct PartialExit {
        //uint8 opType; -- present in pubdata, ignored at serialization
        //uint32 accountId; -- present in pubdata, ignored at serialization
        uint32 tokenId;
        uint128 amount;
        //uint16 fee; -- present in pubdata, ignored at serialization
        address owner;
    }

    function readPartialExitPubdata(bytes memory _data) internal pure returns (PartialExit memory parsed) {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint256 offset = OP_TYPE_BYTES + ACCOUNT_ID_BYTES; // opType + accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt32(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        offset += FEE_BYTES; // fee (ignored)
        (offset, parsed.owner) = Bytes.readAddress(_data, offset); // owner
    }

    // ForcedExit pubdata

    struct ForcedExit {
        //uint8 opType; -- present in pubdata, ignored at serialization
        //uint32 initiatorAccountId; -- present in pubdata, ignored at serialization
        //uint32 targetAccountId; -- present in pubdata, ignored at serialization
        uint32 tokenId;
        uint128 amount;
        //uint16 fee; -- present in pubdata, ignored at serialization
        address target;
    }

    function readForcedExitPubdata(bytes memory _data) internal pure returns (ForcedExit memory parsed) {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint256 offset = OP_TYPE_BYTES + ACCOUNT_ID_BYTES * 2; // opType + initiatorAccountId + targetAccountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt32(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        offset += FEE_BYTES; // fee (ignored)
        (offset, parsed.target) = Bytes.readAddress(_data, offset); // target
    }

    // ChangePubKey

    enum ChangePubkeyType {
        ECRECOVER,
        CREATE2,
        ECRECOVERV2
    }

    struct ChangePubKey {
        // uint8 opType; -- present in pubdata, ignored at serialization
        uint32 accountId;
        bytes20 pubKeyHash;
        address owner;
        uint16 group;
        uint32 nonce;
        //uint32 tokenId; -- present in pubdata, ignored at serialization
        //uint16 fee; -- present in pubdata, ignored at serialization
    }

    function readChangePubKeyPubdata(bytes memory _data) internal pure returns (ChangePubKey memory parsed) {
        uint256 offset = OP_TYPE_BYTES;
        (offset, parsed.accountId) = Bytes.readUInt32(_data, offset); // accountId
        (offset, parsed.pubKeyHash) = Bytes.readBytes20(_data, offset); // pubKeyHash
        (offset, parsed.owner) = Bytes.readAddress(_data, offset); // owner
        (offset, parsed.nonce) = Bytes.readUInt32(_data, offset); // nonce
    }

    struct WithdrawNFT {
        //uint8 opType; -- present in pubdata, ignored at serialization
        //uint32 accountId; -- present in pubdata, ignored at serialization
        uint32 creatorAccountId;
        address creatorAddress;
        uint32 serialId;
        bytes32 contentHash;
        address receiver;
        uint32 tokenId;
        //uint32 feeTokenId;
        //uint16 fee; -- present in pubdata, ignored at serialization
    }

    function readWithdrawNFTPubdata(bytes memory _data) internal pure returns (WithdrawNFT memory parsed) {
        uint256 offset = OP_TYPE_BYTES + ACCOUNT_ID_BYTES; // opType + accountId (ignored)
        (offset, parsed.creatorAccountId) = Bytes.readUInt32(_data, offset);
        (offset, parsed.creatorAddress) = Bytes.readAddress(_data, offset);
        (offset, parsed.serialId) = Bytes.readUInt32(_data, offset);
        (offset, parsed.contentHash) = Bytes.readBytes32(_data, offset);
        (offset, parsed.receiver) = Bytes.readAddress(_data, offset);
        (offset, parsed.tokenId) = Bytes.readUInt32(_data, offset);
    }

    // ChangeGroup pubdata

    struct ChangeGroup {
        //uint8 opType; -- present in pubdata, ignored at serialization
        //uint32 accountId; -- present in pubdata, ignored at serialization
        uint32 tokenId;
        uint128 amount;
        //uint16 fee; -- present in pubdata, ignored at serialization
        address owner;
        uint16 group_destination;
    }

    function readChangeGroupPubdata(bytes memory _data) internal pure returns (ChangeGroup memory parsed) {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint256 offset = OP_TYPE_BYTES + ACCOUNT_ID_BYTES; // opType + accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt32(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        offset += FEE_BYTES; // fee (ignored)
        (offset, parsed.owner) = Bytes.readAddress(_data, offset); // owner
        (offset, parsed.group_destination) = Bytes.readUInt16(_data, offset); // group
    }

    struct FullChangeGroup {
        // uint8 opType
        uint32 accountId;
        address owner;
        uint32 tokenId;
        uint16 group1;
        uint16 group2;
        uint128 amount;
        uint32 nftCreatorAccountId;
        address nftCreatorAddress;
        uint32 nftSerialId;
        bytes32 nftContentHash;
    }

    uint256 public constant PACKED_FULL_CHANGE_GROUP_PUBDATA_BYTES =
        OP_TYPE_BYTES +
            ACCOUNT_ID_BYTES +
            ADDRESS_BYTES +
            TOKEN_BYTES +
            AMOUNT_BYTES +
            ACCOUNT_ID_BYTES +
            ADDRESS_BYTES +
            NFT_SERIAL_ID_BYTES +
            CONTENT_HASH_BYTES +
            GROUP_BYTES *
            2;

    function readFullChangeGroupPubdata(bytes memory _data) internal pure returns (FullChangeGroup memory parsed) {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint256 offset = OP_TYPE_BYTES;
        (offset, parsed.accountId) = Bytes.readUInt32(_data, offset); // accountId
        (offset, parsed.owner) = Bytes.readAddress(_data, offset); // owner
        (offset, parsed.tokenId) = Bytes.readUInt32(_data, offset); // tokenId
        (offset, parsed.group1) = Bytes.readUInt16(_data, offset); // group1
        (offset, parsed.group2) = Bytes.readUInt16(_data, offset); // group2
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        (offset, parsed.nftCreatorAccountId) = Bytes.readUInt32(_data, offset); // nftCreatorAccountId
        (offset, parsed.nftCreatorAddress) = Bytes.readAddress(_data, offset); // nftCreatorAddress
        (offset, parsed.nftSerialId) = Bytes.readUInt32(_data, offset); // nftSerialId
        (offset, parsed.nftContentHash) = Bytes.readBytes32(_data, offset); // nftContentHash
        require(offset == PACKED_FULL_CHANGE_GROUP_PUBDATA_BYTES, "O"); // reading invalid full exit pubdata size
    }

    function writeFullChangeGroupPubdataForPriorityQueue(FullChangeGroup memory op)
        internal
        pure
        returns (bytes memory buf)
    {
        buf = abi.encodePacked(
            uint8(OpType.FullChangeGroup),
            op.accountId, // accountId
            op.owner, // owner
            op.tokenId, // tokenId
            op.group1, // group1
            op.group2, //group2
            uint128(0), // amount -- ignored
            uint32(0), // nftCreatorAccountId -- ignored
            address(0), // nftCreatorAddress -- ignored
            uint32(0), // nftSerialId -- ignored
            bytes32(0) // nftContentHash -- ignored
        );
    }

    function checkFullChangeGroupInPriorityQueue(FullChangeGroup memory op, bytes20 hashedPubdata)
        internal
        pure
        returns (bool)
    {
        return Utils.hashBytesToBytes20(writeFullChangeGroupPubdataForPriorityQueue(op)) == hashedPubdata;
    }
}
