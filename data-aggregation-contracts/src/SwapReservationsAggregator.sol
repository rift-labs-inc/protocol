// NOT ACTUALLY USED BY THE PROTOCOL (HELPER FOR FRONTEND)

// SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

contract SwapReservationsAggregator {
    constructor(uint32[] memory indexesArray, address riftExchangeContract) {
        bytes[] memory allSwapReservations = new bytes[](indexesArray.length);

        for (uint256 i = 0; i < indexesArray.length; ++i) {
            (
                bool success,
                bytes memory swapReservationData
            ) = riftExchangeContract.call{gas: 10_000_000}(
                    abi.encodeWithSignature(
                        "getReservation(uint256)",
                        indexesArray[i]
                    )
                );
            require(
                success,
                "SwapReservationsAggregator: failed to get reservation"
            );
            allSwapReservations[i] = swapReservationData;
        }

        bytes memory _abiEncodedData = abi.encode(allSwapReservations);

        assembly {
            let dataStart := add(_abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }
}
