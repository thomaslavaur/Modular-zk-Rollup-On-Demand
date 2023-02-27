// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";
import "../AdditionalZkSync.sol";

contract ZkSyncRegenesisTest is ZkSync {
    function getStoredBlockHash() external view returns (bytes32) {
        require(
            uint_to_group[address_to_int[msg.sender]].totalBlocksCommitted ==
                uint_to_group[address_to_int[msg.sender]].totalBlocksProven,
            "wq1"
        ); // All the blocks must be processed
        require(
            uint_to_group[address_to_int[msg.sender]].totalBlocksCommitted ==
                uint_to_group[address_to_int[msg.sender]].totalBlocksExecuted,
            "w12"
        ); // All the blocks must be processed

        return
            uint_to_group[address_to_int[msg.sender]].storedBlockHashes[
                uint_to_group[address_to_int[msg.sender]].totalBlocksExecuted
            ];
    }

    function getAdditionalZkSync() external view returns (AdditionalZkSync) {
        return additionalZkSync;
    }
}
