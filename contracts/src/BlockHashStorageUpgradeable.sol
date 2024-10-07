// SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.27;

import "@openzeppelin-upgradeable/proxy/utils/Initializable.sol";

error InvalidSafeBlock();
error InvalidBlockHeights();
error BlockDoesNotExist();
error InvalidConfirmationBlock();
error InvalidProposedBlockOverwrite();
error BlockArraysMismatch();
error InvalidChainwork();

contract BlockHashStorageUpgradeable is Initializable {
    uint8 constant MINIMUM_CONFIRMATION_DELTA = 1;

    mapping(uint256 => bytes32) blockchain; // block height => block hash
    mapping(uint256 => uint256) chainworks; // block height => chainwork
    uint256 public currentHeight;

    event BlocksAdded(uint256 startBlockHeight, uint256 count);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function __BlockHashStorageUpgradeable_init(
        uint256 safeBlockHeight,
        uint256 safeBlockChainwork,
        bytes32 safeBlockHash,
        bytes32 retargetBlockHash
    ) internal onlyInitializing {
        currentHeight = safeBlockHeight;
        chainworks[safeBlockHeight] = safeBlockChainwork;
        blockchain[safeBlockHeight] = safeBlockHash;
        blockchain[calculateRetargetHeight(safeBlockHeight)] = retargetBlockHash;
    }

    enum AddBlockReturn {
        UNMODIFIED,
        MODIFIED
    }

    // Assumes that all blockHashes passed are in a chain as proven by the circuit
    function addBlock(
        uint256 safeBlockHeight,
        uint256 proposedBlockHeight,
        uint256 confirmationBlockHeight,
        bytes32[] memory blockHashes, // from safe block to confirmation block
        uint256[] memory blockChainworks
    ) internal returns (AddBlockReturn) {
        uint256 tipBlockHeight = currentHeight;
        uint256 tipChainwork = chainworks[currentHeight];

        // [1] ensure safe < proposed < confirmation
        if (safeBlockHeight >= proposedBlockHeight || proposedBlockHeight >= confirmationBlockHeight) {
            revert InvalidBlockHeights();
        }

        // [3] ensure safeBlockHeight exists in the contract ( ≠ bytes32(0) )
        if (blockchain[safeBlockHeight] == bytes32(0)) {
            revert InvalidSafeBlock();
        }

        // [0] ensure arrays are same length && matches delta between confirmationBlockHeight-safeBlockHeight
        if (
            blockHashes.length != blockChainworks.length ||
            blockHashes.length != (confirmationBlockHeight - safeBlockHeight) + 1
        ) {
            revert BlockArraysMismatch();
        }

        // [2] ensure confirmationBlockHeight - proposedBlockHeight is >= minimumConfirmationDelta
        if (confirmationBlockHeight - proposedBlockHeight < MINIMUM_CONFIRMATION_DELTA) {
            revert InvalidConfirmationBlock();
        }

        uint256 confirmationBlockIndex = blockHashes.length - 1;
        uint256 proposedBlockIndex = proposedBlockHeight - safeBlockHeight;

        // [4] return if prposed block already exists and matches
        if (blockchain[proposedBlockHeight] == blockHashes[proposedBlockIndex]) {
            return AddBlockReturn.UNMODIFIED;
        }

        // [5] handle block addition/overwrites if you have longer chainwork than tip
        if (blockChainworks[confirmationBlockIndex] > tipChainwork) {
            // [0] fill in all blocks/chainwork from safeBlockHeight + 1 to confirmationBlockHeight
            for (uint256 i = safeBlockHeight + 1; i <= confirmationBlockHeight; i++) {
                blockchain[i] = blockHashes[i - safeBlockHeight];
                chainworks[i] = blockChainworks[i - safeBlockHeight];
            }

            // [1] clear out everything past your confirmation block if you have longer chainwork than tip
            if (confirmationBlockHeight < tipBlockHeight) {
                for (uint256 i = confirmationBlockHeight + 1; i <= tipBlockHeight; i++) {
                    blockchain[i] = bytes32(0);
                    chainworks[i] = uint256(0);
                }
            }

            currentHeight = confirmationBlockHeight;
            emit BlocksAdded(safeBlockHeight, blockHashes.length);
            return AddBlockReturn.MODIFIED;
        }
        // [6] revert if confirmation block chainwork is less than tip chainwork
        revert InvalidChainwork();
    }

    function validateBlockExists(uint256 blockHeight) public view {
        // check if block exists
        if (blockchain[blockHeight] == bytes32(0)) {
            revert BlockDoesNotExist();
        }
    }

    function getBlockHash(uint256 blockHeight) public view returns (bytes32) {
        return blockchain[blockHeight];
    }

    function getChainwork(uint256 blockHeight) public view returns (uint256) {
        return chainworks[blockHeight];
    }

    function calculateRetargetHeight(uint256 blockHeight) public pure returns (uint256) {
        return blockHeight - (blockHeight % 2016);
    }
}
