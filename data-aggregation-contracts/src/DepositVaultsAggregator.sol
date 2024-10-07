// NOT ACTUALLY USED BY THE PROTOCOL (HELPER FOR FRONTEND)

// SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

contract DepositVaultsAggregator {
    constructor(uint32[] memory indexesArray, address riftExchangeContract) {
        bytes[] memory allDepositVaults = new bytes[](indexesArray.length);

        for (uint256 i = 0; i < indexesArray.length; ++i) {
            (, bytes memory depositVaultData) = riftExchangeContract.call{
                gas: 20010
            }(
                abi.encodeWithSignature(
                    "getDepositVault(uint256)",
                    indexesArray[i]
                )
            );
            allDepositVaults[i] = depositVaultData;
        }

        bytes memory _abiEncodedData = abi.encode(allDepositVaults);

        assembly {
            let dataStart := add(_abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }
}
