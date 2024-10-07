// NOT ACTUALLY USED BY THE PROTOCOL (HELPER FOR HYPERNODE)

// SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

contract BlockHeaderAggregator {
    constructor(uint256[] memory heights, address riftExchangeContract) {
        bytes[] memory blocks = new bytes[](heights.length);

        for (uint256 i = 0; i < heights.length; ++i) {
            (, bytes memory _block) = riftExchangeContract.call{
                gas: 30000
            }(
                abi.encodeWithSignature(
                    "getBlockHash(uint256)",
                    heights[i]
                )
            );
            blocks[i] = _block;
        }

        bytes memory _abiEncodedData = abi.encode(blocks);

        assembly {
            let dataStart := add(_abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }
}
