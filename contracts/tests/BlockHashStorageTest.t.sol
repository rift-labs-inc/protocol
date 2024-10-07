// SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {console} from "forge-std/console.sol";
import {BlockHashStorageUpgradeable} from "../src/BlockHashStorageUpgradeable.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {TestBlocks} from "./TestBlocks.sol";

// Exposes the internal block hash storage functions for testing
contract BlockHashProxy is BlockHashStorageUpgradeable {
    function initialize(
        uint256 initialCheckpointHeight,
        uint256 currentChainwork,
        bytes32 initialBlockHash,
        bytes32 initialRetargetBlockHash
    ) public initializer {
        __BlockHashStorageUpgradeable_init(
            initialCheckpointHeight,
            currentChainwork,
            initialBlockHash,
            initialRetargetBlockHash
        );
    }

    function AddBlock(
        uint256 safeBlockHeight,
        uint256 proposedBlockHeight,
        uint256 confirmationBlockHeight,
        bytes32[] memory blockHashes,
        uint256[] memory blockChainworks
    ) public returns (BlockHashStorageUpgradeable.AddBlockReturn) {
        return addBlock(safeBlockHeight, proposedBlockHeight, confirmationBlockHeight, blockHashes, blockChainworks);
    }

    function getMinimumConfirmationDelta() public pure returns (uint8) {
        return MINIMUM_CONFIRMATION_DELTA;
    }

    function getCurrentHeight() public view returns (uint256) {
        return currentHeight;
    }
}

error InvalidSafeBlock();
error InvalidBlockHeights();
error BlockDoesNotExist();
error InvalidConfirmationBlock();
error InvalidProposedBlockOverwrite();
error BlockArraysMismatch();
error InvalidChainwork();

contract BlockHashStorageTest is Test, TestBlocks {
    bytes4 constant INVALID_SAFE_BLOCK = bytes4(keccak256("InvalidSafeBlock()"));
    bytes4 constant INVALID_BLOCK_HEIGHTS = bytes4(keccak256("InvalidBlockHeights()"));
    bytes4 constant BLOCK_DOES_NOT_EXIST = bytes4(keccak256("BlockDoesNotExist()"));
    bytes4 constant INVALID_CONFIRMATION_BLOCK = bytes4(keccak256("InvalidConfirmationBlock()"));
    bytes4 constant INVALID_PROPOSED_BLOCK_OVERWRITE = bytes4(keccak256("InvalidProposedBlockOverwrite()"));
    bytes4 constant BLOCK_ARRAYS_MISMATCH = bytes4(keccak256("BlockArraysMismatch()"));
    bytes4 constant INVALID_CHAINWORK = bytes4(keccak256("InvalidChainwork()"));

    BlockHashProxy public blockHashProxy;
    address public proxyAddress;
    uint256 initialCheckpointHeight;
    uint8 minimumConfirmationDelta;

    function setUp() public {
        minimumConfirmationDelta = 2;
        initialCheckpointHeight = blockHeights[0];

        // Deploy the implementation contract
        BlockHashProxy implementation = new BlockHashProxy();

        // Prepare initialization data
        bytes memory initData = abi.encodeWithSelector(
            BlockHashProxy.initialize.selector,
            initialCheckpointHeight,
            blockChainworks[0],
            blockHashes[0],
            retargetBlockHash,
            minimumConfirmationDelta
        );

        // Deploy the proxy contract
        ERC1967Proxy proxy = new ERC1967Proxy(address(implementation), initData);

        // Set the proxyAddress
        proxyAddress = address(proxy);

        // Create an instance of BlockHashProxy pointing to the proxy contract
        blockHashProxy = BlockHashProxy(proxyAddress);
    }

    /// @notice Test adding valid blocks and updating currentHeight
    function testAddValidBlocks() public {
        uint256 safeBlockHeight = blockHeights[0]; // Initial safe block height
        uint256 proposedBlockHeight = blockHeights[5]; // Proposed block height
        uint256 confirmationBlockHeight = blockHeights[10]; // Confirmation block height

        // includes safe and conf blocks
        uint8 numBlocks = 11;

        // Prepare arrays of block hashes and chainworks
        bytes32[] memory hashes = new bytes32[](numBlocks);
        uint256[] memory chainworks = new uint256[](numBlocks);
        for (uint256 i = 1; i <= numBlocks; i++) {
            hashes[i - 1] = blockHashes[i];
            chainworks[i - 1] = blockChainworks[i];
        }

        // Add blocks
        blockHashProxy.AddBlock(safeBlockHeight, proposedBlockHeight, confirmationBlockHeight, hashes, chainworks);

        // Verify that blocks have been added correctly
        for (uint256 i = safeBlockHeight + 1; i <= confirmationBlockHeight; i++) {
            bytes32 storedHash = blockHashProxy.getBlockHash(i);
            uint256 storedChainwork = blockHashProxy.getChainwork(i);
            assertEq(storedHash, blockHashes[i - initialCheckpointHeight + 1]);
            assertEq(storedChainwork, blockChainworks[i - initialCheckpointHeight + 1]);
        }

        // Verify that currentHeight has been updated
        uint256 currentHeight = blockHashProxy.getCurrentHeight();
        assertEq(currentHeight, confirmationBlockHeight);
    }

    /// @notice Test adding blocks with invalid heights
    function testAddInvalidBlockHeights() public {
        uint256 safeBlockHeight = blockHeights[5]; // Initial safe block height
        uint256 proposedBlockHeight = blockHeights[0]; // Proposed block height, behind safe block
        uint256 confirmationBlockHeight = blockHeights[10]; // Confirmation block height

        // includes safe and conf blocks
        uint8 numBlocks = 11;

        console.log("Safe block height: ", safeBlockHeight);
        console.log("Proposed block height: ", proposedBlockHeight);
        console.log("Confirmation block height: ", confirmationBlockHeight);

        // Prepare arrays of block hashes and chainworks
        bytes32[] memory hashes = new bytes32[](numBlocks);
        uint256[] memory chainworks = new uint256[](numBlocks);
        for (uint256 i = 1; i <= numBlocks; i++) {
            hashes[i - 1] = blockHashes[i];
            chainworks[i - 1] = blockChainworks[i];
        }

        // Add blocks
        vm.expectRevert(INVALID_BLOCK_HEIGHTS);
        blockHashProxy.AddBlock(safeBlockHeight, proposedBlockHeight, confirmationBlockHeight, hashes, chainworks);
    }

    /// @notice Test adding blocks with safe block that does not exist

    function testAddInvalidSafeBlock() public {
        uint256 safeBlockHeightIndex = 1; // 1 here is +1 greater than what is stored
        uint256 proposedBlockHeightIndex = 5;
        uint256 confirmationBlockHeightIndex = 10;
        uint256 safeBlockHeight = blockHeights[safeBlockHeightIndex]; // Initial safe block height
        uint256 proposedBlockHeight = blockHeights[proposedBlockHeightIndex]; // Proposed block height
        uint256 confirmationBlockHeight = blockHeights[confirmationBlockHeightIndex]; // Confirmation block height

        // includes safe and conf blocks
        uint256 numBlocks = confirmationBlockHeightIndex - safeBlockHeightIndex + 1;

        console.log("Safe block height: ", safeBlockHeight);
        console.log("Proposed block height: ", proposedBlockHeight);
        console.log("Confirmation block height: ", confirmationBlockHeight);

        // Prepare arrays of block hashes and chainworks
        bytes32[] memory hashes = new bytes32[](numBlocks);
        uint256[] memory chainworks = new uint256[](numBlocks);
        for (uint256 i = 1; i <= numBlocks; i++) {
            hashes[i - 1] = blockHashes[i];
            chainworks[i - 1] = blockChainworks[i];
        }

        // Add blocks
        vm.expectRevert(INVALID_SAFE_BLOCK);
        blockHashProxy.AddBlock(safeBlockHeight, proposedBlockHeight, confirmationBlockHeight, hashes, chainworks);
    }

    function testBlockArraysMismatch() public {
        uint256 safeBlockHeight = blockHeights[0]; // Initial safe block height
        uint256 proposedBlockHeight = blockHeights[5]; // Proposed block height
        uint256 confirmationBlockHeight = blockHeights[10]; // Confirmation block height

        // Calculate the expected number of blocks
        uint256 expectedNumBlocks = (confirmationBlockHeight - safeBlockHeight) + 1;

        // Create arrays with incorrect length (one less than expected)
        bytes32[] memory hashes = new bytes32[](expectedNumBlocks - 1);
        uint256[] memory chainworks = new uint256[](expectedNumBlocks - 1);

        // Fill arrays with some dummy data
        for (uint256 i = 0; i < hashes.length; i++) {
            hashes[i] = bytes32(i);
            chainworks[i] = i;
        }

        // Attempt to add blocks with mismatched array lengths
        vm.expectRevert(BLOCK_ARRAYS_MISMATCH);
        blockHashProxy.AddBlock(safeBlockHeight, proposedBlockHeight, confirmationBlockHeight, hashes, chainworks);

        // Test with mismatched lengths between hashes and chainworks
        hashes = new bytes32[](expectedNumBlocks);
        chainworks = new uint256[](expectedNumBlocks - 1);

        // Fill arrays with some dummy data
        for (uint256 i = 0; i < chainworks.length; i++) {
            hashes[i] = bytes32(i);
            chainworks[i] = i;
        }
        hashes[chainworks.length] = bytes32(chainworks.length); // Add one more to hashes

        // Attempt to add blocks with mismatched array lengths
        vm.expectRevert(BLOCK_ARRAYS_MISMATCH);
        blockHashProxy.AddBlock(safeBlockHeight, proposedBlockHeight, confirmationBlockHeight, hashes, chainworks);
    }

    function testExactMinimumConfirmationDelta() public {
        uint256 safeBlockHeight = blockHeights[0]; // Initial safe block height
        uint256 proposedBlockHeight = blockHeights[5]; // Proposed block height
        uint8 minimumConfirmationDelta = blockHashProxy.getMinimumConfirmationDelta();
        uint256 confirmationBlockHeight = proposedBlockHeight + minimumConfirmationDelta;

        // Prepare valid block data
        bytes32[] memory hashes = new bytes32[](confirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory chainworks = new uint256[](confirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < hashes.length; i++) {
            hashes[i] = blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1];
            chainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1];
        }

        blockHashProxy.AddBlock(safeBlockHeight, proposedBlockHeight, confirmationBlockHeight, hashes, chainworks);

        // Verify that the block was added successfully
        assertEq(blockHashProxy.getCurrentHeight(), confirmationBlockHeight);
    }

    function testGreaterThanMinimumConfirmationDelta() public {
        uint256 safeBlockHeight = blockHeights[0]; // Initial safe block height
        uint256 proposedBlockHeight = blockHeights[5]; // Proposed block height
        uint8 minimumConfirmationDelta = blockHashProxy.getMinimumConfirmationDelta();
        uint256 confirmationBlockHeight = proposedBlockHeight + minimumConfirmationDelta + 1;

        // Prepare valid block data
        bytes32[] memory hashes = new bytes32[](confirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory chainworks = new uint256[](confirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < hashes.length; i++) {
            hashes[i] = blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1];
            chainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1];
        }

        blockHashProxy.AddBlock(safeBlockHeight, proposedBlockHeight, confirmationBlockHeight, hashes, chainworks);

        // Verify that the block was added successfully
        assertEq(blockHashProxy.getCurrentHeight(), confirmationBlockHeight);
    }

    function testLessThanMinimumConfirmationDelta() public {
        uint256 safeBlockHeight = blockHeights[0]; // Initial safe block height
        uint256 proposedBlockHeight = blockHeights[5]; // Proposed block height
        uint8 minimumConfirmationDelta = blockHashProxy.getMinimumConfirmationDelta();

        // Set confirmation block height equal to proposed block height (delta of 0)
        uint256 invalidConfirmationBlockHeight = proposedBlockHeight;

        console.log("Minimum confirmation delta:", minimumConfirmationDelta);
        console.log("Safe block height:", safeBlockHeight);
        console.log("Proposed block height:", proposedBlockHeight);
        console.log("Invalid confirmation block height:", invalidConfirmationBlockHeight);

        // Prepare block data
        bytes32[] memory hashes = new bytes32[](invalidConfirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory chainworks = new uint256[](invalidConfirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < hashes.length; i++) {
            hashes[i] = blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1];
            chainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1];
        }

        // Expect the transaction to revert with InvalidBlockHeights error
        vm.expectRevert(INVALID_BLOCK_HEIGHTS);
        blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            invalidConfirmationBlockHeight,
            hashes,
            chainworks
        );

        // Verify that the current height hasn't changed
        assertEq(blockHashProxy.getCurrentHeight(), initialCheckpointHeight);
    }

    function testProposedBlockAlreadyExists() public {
        uint256 safeBlockHeight = blockHeights[0];
        uint256 proposedBlockHeight = blockHeights[5];
        uint256 confirmationBlockHeight = blockHeights[10];

        // First, add the initial set of blocks
        bytes32[] memory initialHashes = new bytes32[](confirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory initialChainworks = new uint256[](confirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < initialHashes.length; i++) {
            initialHashes[i] = blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1];
            initialChainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1];
        }

        blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            confirmationBlockHeight,
            initialHashes,
            initialChainworks
        );

        bytes32[] memory newHashes = new bytes32[](confirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory newChainworks = new uint256[](confirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < newHashes.length; i++) {
            newHashes[i] = blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1];
            newChainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1];
        }

        BlockHashProxy.AddBlockReturn chain_update = blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            confirmationBlockHeight,
            newHashes,
            newChainworks
        );
        assertEq(uint8(chain_update), uint8(BlockHashStorageUpgradeable.AddBlockReturn.UNMODIFIED));

        // Verify that the current height hasn't changed
        assertEq(blockHashProxy.getCurrentHeight(), confirmationBlockHeight);
    }

    function testOverwriteExistingBlocks() public {
        uint256 safeBlockHeight = blockHeights[0];
        uint256 proposedBlockHeight = blockHeights[5];
        uint256 confirmationBlockHeight = blockHeights[10];

        // Add initial set of blocks
        bytes32[] memory initialHashes = new bytes32[](confirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory initialChainworks = new uint256[](confirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < initialHashes.length; i++) {
            initialHashes[i] = blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1];
            initialChainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1];
        }

        blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            confirmationBlockHeight,
            initialHashes,
            initialChainworks
        );

        // Prepare new set of blocks with higher chainwork
        bytes32[] memory newHashes = new bytes32[](confirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory newChainworks = new uint256[](confirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < newHashes.length; i++) {
            newHashes[i] = keccak256(abi.encodePacked(blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1]));
            newChainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 15];
        }

        BlockHashProxy.AddBlockReturn result = blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            confirmationBlockHeight,
            newHashes,
            newChainworks
        );

        assertEq(uint8(result), uint8(BlockHashStorageUpgradeable.AddBlockReturn.MODIFIED));

        // Verify that blocks have been overwritten
        for (uint256 i = safeBlockHeight + 1; i <= confirmationBlockHeight; i++) {
            assertEq(blockHashProxy.getBlockHash(i), newHashes[i - safeBlockHeight]);
            assertEq(blockHashProxy.getChainwork(i), newChainworks[i - safeBlockHeight]);
        }
    }

    function testClearBlocksPastConfirmation() public {
        uint256 safeBlockHeight = blockHeights[0];
        uint256 proposedBlockHeight = blockHeights[5];
        uint256 initialConfirmationBlockHeight = blockHeights[15];

        // Add initial set of blocks
        bytes32[] memory initialHashes = new bytes32[](initialConfirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory initialChainworks = new uint256[](initialConfirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < initialHashes.length; i++) {
            initialHashes[i] = blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1];
            initialChainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1];
        }

        blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            initialConfirmationBlockHeight,
            initialHashes,
            initialChainworks
        );

        // Prepare new set of blocks with higher chainwork but shorter chain
        uint256 newConfirmationBlockHeight = blockHeights[10];
        bytes32[] memory newHashes = new bytes32[](newConfirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory newChainworks = new uint256[](newConfirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < newHashes.length; i++) {
            newHashes[i] = keccak256(abi.encodePacked(blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1]));
            newChainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 15]; // Higher chainwork
        }

        BlockHashProxy.AddBlockReturn result = blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            newConfirmationBlockHeight,
            newHashes,
            newChainworks
        );

        assertEq(uint8(result), uint8(BlockHashStorageUpgradeable.AddBlockReturn.MODIFIED));

        // Verify that blocks have been overwritten up to newConfirmationBlockHeight
        for (uint256 i = safeBlockHeight + 1; i <= newConfirmationBlockHeight; i++) {
            assertEq(blockHashProxy.getBlockHash(i), newHashes[i - safeBlockHeight]);
            assertEq(blockHashProxy.getChainwork(i), newChainworks[i - safeBlockHeight]);
        }

        // Verify that blocks past newConfirmationBlockHeight have been cleared
        for (uint256 i = newConfirmationBlockHeight + 1; i <= initialConfirmationBlockHeight; i++) {
            assertEq(blockHashProxy.getBlockHash(i), bytes32(0));
            assertEq(blockHashProxy.getChainwork(i), 0);
        }

        // Verify that currentHeight has been updated
        assertEq(blockHashProxy.getCurrentHeight(), newConfirmationBlockHeight);
    }

    function testRevertOnLowerChainwork() public {
        uint256 safeBlockHeight = blockHeights[0];
        uint256 proposedBlockHeight = blockHeights[5];
        uint256 confirmationBlockHeight = blockHeights[10];

        // Add initial set of blocks
        bytes32[] memory initialHashes = new bytes32[](confirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory initialChainworks = new uint256[](confirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < initialHashes.length; i++) {
            initialHashes[i] = blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1];
            initialChainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1];
        }

        blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            confirmationBlockHeight,
            initialHashes,
            initialChainworks
        );

        // Prepare new set of blocks with lower chainwork
        bytes32[] memory newHashes = new bytes32[](confirmationBlockHeight - safeBlockHeight + 1);
        uint256[] memory newChainworks = new uint256[](confirmationBlockHeight - safeBlockHeight + 1);
        for (uint256 i = 0; i < newHashes.length; i++) {
            newHashes[i] = keccak256(abi.encodePacked(blockHashes[i + safeBlockHeight - initialCheckpointHeight + 1]));
            newChainworks[i] = blockChainworks[i + safeBlockHeight - initialCheckpointHeight + 1] - 1000000; // Lower chainwork
        }

        vm.expectRevert(INVALID_CHAINWORK);
        blockHashProxy.AddBlock(
            safeBlockHeight,
            proposedBlockHeight,
            confirmationBlockHeight,
            newHashes,
            newChainworks
        );
    }
}
