// SPDX-License-Identifier: MIT OR Apache-2.0
// solhint-disable max-states-count

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "./IERC20.sol";

import "./Governance.sol";
import "./Verifier.sol";
import "./Operations.sol";
import "./NFTFactory.sol";
import "./AdditionalZkSync.sol";

/// @title zkSync storage contract
/// @author Matter Labs
contract Storage {
    /// @dev Governance contract. Contains the governor (the owner) of whole system, validators list, possible tokens list
    Governance internal governance;

    uint8 internal constant FILLED_GAS_RESERVE_VALUE = 0xff; // we use it to set gas revert value so slot will not be emptied with 0 balance
    struct PendingBalance {
        uint128 balanceToWithdraw;
        uint8 gasReserveValue; // gives user opportunity to fill storage slot with nonzero value
    }

    /// @dev Root-chain balances (per owner and token id, see packAddressAndTokenId) to withdraw
    mapping(bytes22 => PendingBalance) internal pendingBalances;

    // Flag indicates that upgrade preparation status is active
    // Will store false in case of not active upgrade mode
    bool upgradePreparationActive;
    // Upgrade preparation activation timestamp (as seconds since unix epoch)
    // Will be equal to zero in case of not active upgrade mode
    uint256 upgradePreparationActivationTime;

    struct Group {
        // Group ID
        uint16 group_id;
        // Verifier contract. Used to verify block proof and exit proof
        Verifier verifier;
        // Total number of executed blocks i.e. blocks[totalBlocksExecuted] points at the latest executed block (block 0 is genesis)
        uint32 totalBlocksExecuted;
        //  Total number of committed blocks i.e. blocks[totalBlocksCommitted] points at the latest committed block
        uint32 totalBlocksCommitted;
        // Flag indicates that a user has exited in the exodus mode certain token balance (per account id and tokenId)
        mapping(uint32 => mapping(uint32 => bool)) performedExodus;
        // Flag indicates that exodus (mass exit) mode is triggered
        // Once it was raised, it can not be cleared again, and all users must exit
        bool exodusMode;
        // First open priority request id
        uint64 firstPriorityRequestId;
        // Total number of requests
        uint64 totalOpenPriorityRequests;
        // Total number of committed requests.
        // Used in checks: if the request matches the operation on Rollup contract and if provided number of requests is not too big
        uint64 totalCommittedPriorityRequests;
        // Stored hashed StoredBlockInfo for some block number
        mapping(uint32 => bytes32) storedBlockHashes;
        // Total blocks proven.
        uint32 totalBlocksProven;
        // Priority Requests mapping (request id - operation)
        // Contains op type, pubdata and expiration block of unsatisfied requests.
        // Numbers are in order of requests receiving
        mapping(uint64 => PriorityOperation) priorityRequests;
        bool permissioned;
        mapping(address => bool) whitelist;
    }
    mapping(address => uint16) internal address_to_int;
    mapping(uint16 => Group) internal uint_to_group;

    /// @dev User authenticated fact hashes for some nonce.
    mapping(address => mapping(uint32 => bytes32)) public authFacts;

    /// @notice Packs address and token id into single word to use as a key in balances mapping
    function packAddressAndTokenId(address _address, uint16 _tokenId) internal pure returns (bytes22) {
        return bytes22((uint176(_address) | (uint176(_tokenId) << 160)));
    }

    /// @Rollup block stored data
    /// @member blockNumber Rollup block number
    /// @member priorityOperations Number of priority operations processed
    /// @member pendingOnchainOperationsHash Hash of all operations that must be processed after verify
    /// @member timestamp Rollup block timestamp, have the same format as Ethereum block constant
    /// @member stateHash Root hash of the rollup state
    /// @member commitment Verified input for the zkSync circuit
    struct StoredBlockInfo {
        uint32 blockNumber;
        uint64 priorityOperations;
        bytes32 pendingOnchainOperationsHash;
        uint256 timestamp;
        bytes32 stateHash;
        bytes32 commitment;
    }

    /// @notice Returns the keccak hash of the ABI-encoded StoredBlockInfo
    function hashStoredBlockInfo(StoredBlockInfo memory _storedBlockInfo) internal pure returns (bytes32) {
        return keccak256(abi.encode(_storedBlockInfo));
    }

    /// @notice Priority Operation container
    /// @member hashedPubData Hashed priority operation public data
    /// @member expirationBlock Expiration block number (ETH block) for this request (must be satisfied before)
    /// @member opType Priority operation type
    struct PriorityOperation {
        bytes20 hashedPubData;
        uint64 expirationBlock;
        Operations.OpType opType;
    }

    /// @dev Timer for authFacts entry reset (address, nonce -> timer).
    /// @dev Used when user wants to reset `authFacts` for some nonce.
    mapping(address => mapping(uint32 => uint256)) internal authFactsResetTimer;

    mapping(uint32 => address) internal withdrawnNFTs;

    mapping(uint32 => Operations.WithdrawNFT) internal pendingWithdrawnNFTs;

    AdditionalZkSync internal additionalZkSync;

    /// @dev Upgrade notice period, possibly shorten by the security council
    uint256 internal approvedUpgradeNoticePeriod;

    /// @dev Upgrade start timestamp (as seconds since unix epoch)
    /// @dev Will be equal to zero in case of not active upgrade mode
    uint256 internal upgradeStartTimestamp;

    /// @dev Stores boolean flags which means the confirmations of the upgrade for each member of security council
    /// @dev Will store zeroes in case of not active upgrade mode
    mapping(uint256 => bool) internal securityCouncilApproves;
    uint256 internal numberOfApprovalsFromSecurityCouncil;

    /// @notice Checks that current state not is exodus mode
    function requireActive(uint16 group_id) internal view {
        require(!uint_to_group[group_id].exodusMode); // exodus mode activated
    }
}
