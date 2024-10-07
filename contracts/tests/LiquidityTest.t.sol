// SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

import { Test } from "forge-std/Test.sol";
import { console } from "forge-std/console.sol";
import { ExchangeTestBase } from "./ExchangeTestBase.t.sol";
import { RiftExchange } from "../src/RiftExchange.sol";

contract LiquidityDepositTest is ExchangeTestBase {
    function testLpReservationHash() public view {
        uint64[] memory expectedSatsOutputArray = new uint64[](1);
        bytes22 btcPayoutLockingScript = hex"0014841b80d2cc75f5345c482af96294d04fdd66b2b7";
        expectedSatsOutputArray[0] = 1230;

        bytes32 vaultHash;

        // [5] check if there is enough liquidity in each deposit vaults to reserve
        for (uint256 i = 0; i < expectedSatsOutputArray.length; i++) {
            console.log("hashable chunk");
            console.logBytes(abi.encode(expectedSatsOutputArray[i], btcPayoutLockingScript, vaultHash));
            // [0] retrieve deposit vault
            vaultHash = sha256(abi.encode(expectedSatsOutputArray[i], btcPayoutLockingScript, vaultHash));
        }

        console.log("Vault hash:");
        console.logBytes32(vaultHash);
    }

    // //--------- DEPOSIT TESTS ---------//
    function testDepositLiquidity() public {
        deal(address(usdt), testAddress, 1_000_000_000_000_000e6); // Mint USDT (6 decimals)
        vm.startPrank(testAddress);

        console.log("Starting deposit liquidity...");
        console.log("testaddress USDT balance: ", usdt.balanceOf(testAddress));

        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;
        uint64 exchangeRate = 2557666;
        uint256 depositAmount = 1_000_000_000_000_000e6; // 1b USDT

        usdt.approve(address(riftExchange), depositAmount);

        uint256 gasBefore = gasleft();
        riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);
        uint256 gasUsed = gasBefore - gasleft();
        console.log("Gas used for deposit:", gasUsed);

        uint256 vaultIndex = riftExchange.getDepositVaultsLength() - 1;
        RiftExchange.DepositVault memory deposit = riftExchange.getDepositVault(vaultIndex);

        assertEq(deposit.initialBalance, depositAmount, "Deposit amount mismatch");
        assertEq(deposit.exchangeRate, exchangeRate, "BTC exchange rate mismatch");

        vm.stopPrank();
    }

    function testDepositMultiple() public {
        uint256 totalAmount = 1_000_000_000_000_000e6; // 1 quadrillion USDT
        deal(address(usdt), testAddress, totalAmount);
        vm.startPrank(testAddress);

        usdt.approve(address(riftExchange), totalAmount);

        uint256 firstDepositGasCost;
        uint256 lastDepositGasCost;

        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;
        uint64 exchangeRate = 2557666;
        uint256 depositAmount = 1_000_000e6; // 1 million USDT per deposit
        uint256 totalGasUsed = 0;

        // create multiple deposits
        uint256 numDeposits = 1000;
        for (uint256 i = 0; i < numDeposits; i++) {
            uint256 gasBefore = gasleft();

            riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);

            uint256 gasUsed = gasBefore - gasleft(); // Calculate gas used for the operation
            totalGasUsed += gasUsed; // Accumulate total gas used

            if (i == 0) {
                firstDepositGasCost = gasUsed; // Store gas cost of the first deposit
            }
            if (i == numDeposits - 1) {
                lastDepositGasCost = gasUsed; // Store gas cost of the last deposit
            }
        }

        uint256 averageGasCost = totalGasUsed / numDeposits; // Calculate the average gas cost

        vm.stopPrank();

        // Output the gas cost for first and last deposits
        console.log("Gas cost for the first deposit:", firstDepositGasCost);
        console.log("Gas cost for the last deposit:", lastDepositGasCost);
        console.log("Average gas cost:", averageGasCost);

        // Assert that all deposits were successful
        assertEq(riftExchange.getDepositVaultsLength(), numDeposits, "Number of deposits mismatch");

        // Check the total amount deposited
        uint256 totalDeposited = depositAmount * numDeposits;
        assertLe(totalDeposited, totalAmount, "Total deposited amount exceeds initial balance");

        // check the balance of a few random vaults
        for (uint256 i = 0; i < 5; i++) {
            uint256 randomIndex = uint256(keccak256(abi.encodePacked(block.timestamp, i))) % numDeposits;
            RiftExchange.DepositVault memory vault = riftExchange.getDepositVault(randomIndex);
            assertEq(vault.initialBalance, depositAmount, "Deposit amount in vault mismatch");
        }
    }

    function testDepositUpdateExchangeRate() public {
        // setup
        uint256 totalAmount = 1_000_000_000e6; // 1 billion USDT
        deal(address(usdt), testAddress, totalAmount);
        vm.startPrank(testAddress);
        usdt.approve(address(riftExchange), totalAmount);

        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;
        uint64 initialBtcExchangeRate = 69;
        uint256 depositAmount = 100_000_000e6; // 100 million USDT

        // create initial deposit
        riftExchange.depositLiquidity(depositAmount, initialBtcExchangeRate, btcPayoutLockingScript);

        // update the BTC exchange rate
        uint64 newBtcExchangeRate = 75;
        console.log("Updating BTC exchange rate from", initialBtcExchangeRate, "to", newBtcExchangeRate);
        uint256[] memory empty = new uint256[](0);
        riftExchange.updateExchangeRate(0, newBtcExchangeRate, empty);
        console.log("NEW BTC EXCHANGE RATE:", riftExchange.getDepositVault(0).exchangeRate);

        // fetch the updated deposit and verify the new exchange rate
        RiftExchange.DepositVault memory updatedDeposit = riftExchange.getDepositVault(0);
        assertEq(
            updatedDeposit.exchangeRate, newBtcExchangeRate, "BTC exchange rate should be updated to the new value"
        );

        // Verify that the deposit amount remains unchanged
        assertEq(
            updatedDeposit.initialBalance,
            depositAmount,
            "Deposit amount should remain unchanged after exchange rate update"
        );

        vm.stopPrank();
    }

    // // --------- RESERVATION TESTS ---------//

    function testReserveLiquidity() public {
        // setup
        uint256 totalAmount = 1_000_000_000e6; // 1 billion USDT
        deal(address(usdt), testAddress, totalAmount);
        vm.startPrank(testAddress);
        usdt.approve(address(riftExchange), totalAmount);
        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;
        uint64 exchangeRate = 69;
        uint192 depositAmount = 500_000_000e6; // 500 million USDT

        // deposit liquidity
        riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);

        // check how much is available in the vault
        uint256 vaultBalance = riftExchange.getDepositVaultUnreservedBalance(0);
        console.log("Vault balance:", vaultBalance);

        // setup for reservation
        uint256[] memory vaultIndexesToReserve = new uint256[](1);
        vaultIndexesToReserve[0] = 0;
        uint192[] memory amountsToReserve = new uint192[](1);
        amountsToReserve[0] = 100_000_000e6; // 100 million USDT
        address ethPayoutAddress = address(0x123); // Example ETH payout address
        uint256[] memory empty = new uint256[](0);

        // Approve additional USDT for fees
        uint256 additionalApproval = 1_000_000e6; // 1 million USDT for fees
        usdt.approve(address(riftExchange), uint256(amountsToReserve[0]) + additionalApproval);

        console.log("Amount trying to reserve:", amountsToReserve[0]);

        uint256 gasBefore = gasleft();
        // usdt balance before
        console.log("USDT balance before reservation:", usdt.balanceOf(testAddress));
        uint256 demoTotalSatsInput = 10000;
        riftExchange.reserveLiquidity(
            msg.sender, vaultIndexesToReserve, amountsToReserve, ethPayoutAddress, demoTotalSatsInput, empty
        );

        // usdt balance after
        console.log("USDT balance after reservation:", usdt.balanceOf(testAddress));
        uint256 gasUsed = gasBefore - gasleft();
        console.log("Gas used for reservation:", gasUsed);

        // fetch reservation to validate
        RiftExchange.SwapReservation memory reservation = riftExchange.getReservation(0);

        // assertions
        assertEq(reservation.ethPayoutAddress, ethPayoutAddress, "ETH payout address should match");
        assertEq(reservation.totalSwapOutputAmount, uint256(amountsToReserve[0]), "Total swap amount should match");

        // validate balances and state changes
        uint256 remainingBalance = riftExchange.getDepositVaultUnreservedBalance(0);

        console.log("Remaining balance:", remainingBalance);
        assertEq(
            remainingBalance,
            uint256(depositAmount) - uint256(amountsToReserve[0]),
            "Vault balance should decrease by the reserved amount"
        );

        vm.stopPrank();
    }

    function testReserveLiquidityMultipleVaults() public {
        // Setup
        uint256 totalAmount = 3_000_000_000e6; // 3 billion USDT
        deal(address(usdt), testAddress, totalAmount);
        vm.startPrank(testAddress);
        usdt.approve(address(riftExchange), totalAmount);
        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;

        // Deposit liquidity into 3 vaults
        uint64[] memory exchangeRates = new uint64[](3);
        exchangeRates[0] = 596302900000000;
        exchangeRates[1] = 596302900000000;
        exchangeRates[2] = 596302900000000;

        uint192 depositAmount = 500_000_000e6; // 500 million USDT per vault

        for (uint256 i = 0; i < 3; i++) {
            riftExchange.depositLiquidity(depositAmount, exchangeRates[i], btcPayoutLockingScript);
        }

        // Setup for reservation
        uint256[] memory vaultIndexesToReserve = new uint256[](3);
        vaultIndexesToReserve[0] = 0;
        vaultIndexesToReserve[1] = 1;
        vaultIndexesToReserve[2] = 2;

        uint192[] memory amountsToReserve = new uint192[](3);
        amountsToReserve[0] = 1e6; // 1 USDT
        amountsToReserve[1] = 1e6; // 1 USDT
        amountsToReserve[2] = 2e6; // 2 USDT

        address ethPayoutAddress = address(0x123); // Example ETH payout address
        uint256[] memory empty = new uint256[](0);

        // Approve additional USDT for fees
        uint256 totalReserveAmount =
            uint256(amountsToReserve[0]) + uint256(amountsToReserve[1]) + uint256(amountsToReserve[2]);
        uint256 additionalApproval = 3_000_000e6; // 3 million USDT for fees
        usdt.approve(address(riftExchange), totalReserveAmount + additionalApproval);

        console.log("Total amount trying to reserve:", totalReserveAmount);

        uint256 gasBefore = gasleft();

        // USDT balance before
        console.log("USDT balance before reservation:", usdt.balanceOf(testAddress));

        uint256 demoTotalSatsInput = 30000; // Increased for multiple vaults

        riftExchange.reserveLiquidity(
            msg.sender, vaultIndexesToReserve, amountsToReserve, ethPayoutAddress, demoTotalSatsInput, empty
        );

        // USDT balance after
        console.log("USDT balance after reservation:", usdt.balanceOf(testAddress));

        uint256 gasUsed = gasBefore - gasleft();
        console.log("Gas used for reservation:", gasUsed);

        // Fetch reservation to validate
        RiftExchange.SwapReservation memory reservation = riftExchange.getReservation(0);

        // Assertions
        assertEq(reservation.ethPayoutAddress, ethPayoutAddress, "ETH payout address should match");
        assertEq(reservation.totalSwapOutputAmount, totalReserveAmount, "Total swap amount should match");

        // Validate balances and state changes
        for (uint256 i = 0; i < 3; i++) {
            uint256 remainingBalance = riftExchange.getDepositVaultUnreservedBalance(i);
            console.log("Remaining balance in vault", i, ":", remainingBalance);
            assertEq(
                remainingBalance,
                uint256(depositAmount) - uint256(amountsToReserve[i]),
                "Vault balance should decrease by the reserved amount"
            );
        }

        vm.stopPrank();
    }

    function testReserveMultipleLiquidity() public {
        // setup
        uint256 totalAmount = 1_000_000_000e6; // 1 billion USDT
        deal(address(usdt), testAddress, totalAmount);
        vm.startPrank(testAddress);
        usdt.approve(address(riftExchange), totalAmount);
        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;
        uint64 exchangeRate = 69;
        uint192 depositAmount = 500_000_000e6; // 500 million USDT

        // deposit liquidity
        riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);

        uint256[] memory vaultIndexesToReserve = new uint256[](1);
        vaultIndexesToReserve[0] = 0;
        uint192[] memory amountsToReserve = new uint192[](1);
        amountsToReserve[0] = 10_000_000e6; // 10 million USDT
        uint256[] memory empty = new uint256[](0);

        // Approve additional USDT for fees
        uint256 additionalApproval = 1_000_000e6; // 1 million USDT for fees
        usdt.approve(address(riftExchange), uint256(amountsToReserve[0]) * 10 + additionalApproval);

        uint256 gasFirst;
        uint256 gasLast;
        uint256 totalGasUsed = 0;
        uint256 numReservations = 10;

        for (uint256 i = 0; i < numReservations; i++) {
            uint256 gasBefore = gasleft();
            uint256 demoTotalSatsInput = 10000;
            riftExchange.reserveLiquidity(
                msg.sender, vaultIndexesToReserve, amountsToReserve, testAddress, demoTotalSatsInput, empty
            );
            uint256 gasUsed = gasBefore - gasleft();
            totalGasUsed += gasUsed;

            if (i == 0) {
                gasFirst = gasUsed;
            } else if (i == numReservations - 1) {
                gasLast = gasUsed;
            }
        }

        uint256 averageGas = totalGasUsed / numReservations;

        console.log("First reservation gas used:", gasFirst);
        console.log("Last reservation gas used:", gasLast);
        console.log("Average gas used for reservations:", averageGas);

        // validate balances and state changes
        uint256 remainingBalance = riftExchange.getDepositVaultUnreservedBalance(0);

        console.log("Remaining balance:", remainingBalance);
        assertEq(
            remainingBalance,
            uint256(depositAmount) - (uint256(amountsToReserve[0]) * numReservations),
            "Vault balance should decrease by the total reserved amount"
        );

        vm.stopPrank();
    }

    function testReservationWithVaryingVaults() public {
        // setup
        uint256 totalAmount = 1_000_000_000e6; // 1 billion USDT
        deal(address(usdt), testAddress, totalAmount);
        vm.startPrank(testAddress);
        usdt.approve(address(riftExchange), totalAmount * 2);

        uint256 maxVaults = 100;
        uint192 depositAmount = 5_000_000e6; // 5 million USDT
        uint64 exchangeRate = 69;
        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;

        // create multiple vaults
        for (uint256 i = 0; i < maxVaults; i++) {
            riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);
        }

        // reserve liquidity from varying vaults
        for (uint256 numVaults = 1; numVaults <= maxVaults; numVaults++) {
            uint256[] memory vaultIndexesToReserve = new uint256[](numVaults);
            uint192[] memory amountsToReserve = new uint192[](numVaults);

            for (uint256 j = 0; j < numVaults; j++) {
                vaultIndexesToReserve[j] = j;
                amountsToReserve[j] = 100e6; // 100 USDT
            }

            // Increase approval to account for fees
            uint256 totalReservation = 110_000e6 * numVaults; // 110,000 USDT per vault (including fees)
            usdt.approve(address(riftExchange), totalReservation);

            uint256[] memory emptyExpiredReservations = new uint256[](0);

            uint256 gasBefore = gasleft();
            uint256 demoTotalSatsInput = 10000;
            riftExchange.reserveLiquidity(
                msg.sender,
                vaultIndexesToReserve,
                amountsToReserve,
                testAddress,
                demoTotalSatsInput,
                emptyExpiredReservations
            );
            uint256 gasUsed = gasBefore - gasleft();
            console.log("Gas used for reserving from", numVaults, "vaults:", gasUsed);
        }

        vm.stopPrank();
    }

    // // //--------- WITHDRAW TESTS ---------//

    function testWithdrawLiquidity() public {
        // setup
        uint256 totalAmount = 5_000_000e6; // 5 million USDT
        deal(address(usdt), testAddress, totalAmount);
        vm.startPrank(testAddress);
        usdt.approve(address(riftExchange), totalAmount);

        // [0] initial deposit
        uint192 depositAmount = 5_000_000e6; // 5 million USDT
        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;
        uint64 exchangeRate = 50;
        riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);

        // Record initial balance
        uint256 initialBalance = usdt.balanceOf(testAddress);

        // [1] withdraw some of the liquidity
        uint256[] memory empty = new uint256[](0);
        uint192 withdrawAmount = 2_000_000e6; // 2 million USDT
        riftExchange.withdrawLiquidity(0, withdrawAmount, empty);

        // [2] check if the balance has decreased correctly
        RiftExchange.DepositVault memory depositAfterWithdrawal = riftExchange.getDepositVault(0);
        uint256 expectedRemaining = uint256(depositAmount) - uint256(withdrawAmount);
        assertEq(
            depositAfterWithdrawal.unreservedBalance,
            expectedRemaining,
            "Remaining deposit should match expected amount after withdrawal"
        );

        // [3] check if the funds reached the LP's address
        uint256 finalBalance = usdt.balanceOf(testAddress);
        assertEq(
            finalBalance,
            initialBalance + uint256(withdrawAmount),
            "LP's balance should increase by the withdrawn amount"
        );

        // check vault withdrawn amount = withdrawAmount
        assertEq(depositAfterWithdrawal.withdrawnAmount, uint256(withdrawAmount), "Withdrawn amount should match");

        vm.stopPrank();
    }

    // // //--------- UPDATE EXCHANGE RATE TESTS --------- //

    function testUpdateExchangeRate() public {
        // setup
        uint256 totalAmount = 10_000_000e6; // 10 million USDT
        deal(address(usdt), testAddress, totalAmount);
        vm.startPrank(testAddress);
        usdt.approve(address(riftExchange), totalAmount);

        // deposit liquidity
        uint192 depositAmount = 5_000_000e6; // 5 million USDT
        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;
        uint64 initialBtcExchangeRate = 50;
        riftExchange.depositLiquidity(depositAmount, initialBtcExchangeRate, btcPayoutLockingScript);

        // update the exchange rate
        uint256 globalVaultIndex = 0;
        uint64 newBtcExchangeRate = 55;
        uint256[] memory expiredReservationIndexes = new uint256[](0);

        riftExchange.updateExchangeRate(globalVaultIndex, newBtcExchangeRate, expiredReservationIndexes);

        // verify new exchange rate
        RiftExchange.DepositVault memory updatedVault = riftExchange.getDepositVault(globalVaultIndex);
        assertEq(updatedVault.exchangeRate, newBtcExchangeRate, "Exchange rate should be updated to the new value.");

        // Verify failure on zero exchange rate
        vm.expectRevert(INVALID_EXCHANGE_RATE);
        riftExchange.updateExchangeRate(globalVaultIndex, 0, expiredReservationIndexes);

        // Test vault forking due to active reservations
        uint256[] memory vaultIndexesToReserve = new uint256[](1);
        vaultIndexesToReserve[0] = globalVaultIndex;
        uint192[] memory amountsToReserve = new uint192[](1);
        amountsToReserve[0] = 1_000_000e6; // 1 million USDT
        uint256 demoTotalSatsInput = 10000;
        riftExchange.reserveLiquidity(
            msg.sender,
            vaultIndexesToReserve,
            amountsToReserve,
            testAddress,
            demoTotalSatsInput,
            expiredReservationIndexes
        );

        riftExchange.updateExchangeRate(globalVaultIndex, 60, expiredReservationIndexes);

        // Verify that a new vault was created
        assertEq(riftExchange.getDepositVaultsLength(), 2, "A new vault should be created");

        // Verify the old vault's state
        RiftExchange.DepositVault memory oldVault = riftExchange.getDepositVault(globalVaultIndex);
        assertEq(oldVault.initialBalance, depositAmount, "Old vault's initial balance should remain unchanged");
        assertEq(oldVault.unreservedBalance, 0, "Old vault's unreserved balance should be 0");
        assertEq(
            oldVault.withdrawnAmount,
            depositAmount - amountsToReserve[0],
            "Old vault's withdrawn amount should be the unreserved balance"
        );
        assertEq(oldVault.exchangeRate, newBtcExchangeRate, "Old vault's exchange rate should remain unchanged");

        // Verify the new vault's state
        RiftExchange.DepositVault memory newVault = riftExchange.getDepositVault(1); // New vault should be at index 1
        assertEq(
            newVault.initialBalance,
            depositAmount - amountsToReserve[0],
            "New vault's initial balance should be the old unreserved balance"
        );
        assertEq(
            newVault.unreservedBalance,
            depositAmount - amountsToReserve[0],
            "New vault's unreserved balance should be the old unreserved balance"
        );
        assertEq(newVault.withdrawnAmount, 0, "New vault's withdrawn amount should be 0");
        assertEq(newVault.exchangeRate, 60, "New vault's exchange rate should be the updated rate");
        assertEq(
            newVault.btcPayoutLockingScript,
            btcPayoutLockingScript,
            "New vault's BTC payout locking script should match the old one"
        );

        vm.stopPrank();
    }

    // //--------- PAUSING DEPOSITS --------- //

    function testPauseDepositLiquidity() public {
        // Setup initial conditions
        uint256 totalAmount = 1_000_000e6; // 1 million USDT
        deal(address(usdt), testAddress, totalAmount); // Mint USDT to testAddress
        vm.startPrank(testAddress); // Start acting as testAddress

        // Approve RiftExchange to spend USDT
        usdt.approve(address(riftExchange), totalAmount);

        // First, deposit liquidity
        bytes22 btcPayoutLockingScript = 0x0014841b80d2cc75f5345c482af96294d04fdd66b2b7;
        uint64 exchangeRate = 596302900000000;
        uint256 depositAmount = 500_000e6; // 500,000 USDT

        riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);

        vm.stopPrank(); // Stop acting as testAddress

        // Now, pause the contract as the owner (address(this))
        riftExchange.updateNewLiquidityDepositsPaused(true);

        // Attempt to deposit liquidity again as testAddress (should fail)
        vm.startPrank(testAddress);
        // Expect the transaction to revert with "Contract is paused"
        vm.expectRevert();
        riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);
        vm.stopPrank();

        // Unpause the contract as the owner
        riftExchange.updateNewLiquidityDepositsPaused(false);

        // Attempt to deposit liquidity again as testAddress (should succeed)
        vm.startPrank(testAddress);
        riftExchange.depositLiquidity(depositAmount, exchangeRate, btcPayoutLockingScript);
        vm.stopPrank();

        // Verify that the total number of deposit vaults is now 2
        uint256 vaultsLength = riftExchange.getDepositVaultsLength();
        assertEq(vaultsLength, 2, "There should be two deposit vaults after unpausing and depositing again");
    }
}
