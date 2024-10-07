// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "forge-std/console.sol";
import "../src/RiftExchange.sol";
import {Upgrades} from "@openzeppelin-foundry-upgrades/Upgrades.sol";

contract UpgradeRiftExchange is Script {
    function run() external {
        vm.startBroadcast();

        console.log("Starting RiftExchange upgrade process...");

        // Address of the existing proxy contract
        address proxyAddress = 0xdc63082C8BfeBc973F2906fbfdB696E88735cCa3;

        console.log("Current proxy address:", proxyAddress);

        // Upgrade the proxy to the new implementation
        Upgrades.upgradeProxy(proxyAddress, "RiftExchange.sol:RiftExchange", "");

        console.log("RiftExchange upgraded successfully");

        vm.stopBroadcast();
    }
}
