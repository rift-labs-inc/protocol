#!/bin/bash

# Helper script to compile contract artifacts and move them to artifacts/ dir 

# Compile contracts
(cd ../contracts && forge compile --via-ir)
(cd ../data-aggregation-contracts && forge compile)

# Clean and create artifacts directory
mkdir -p artifacts
rm -rf artifacts/*

# Copy compiled artifacts
cp ../contracts/out/RiftExchange.sol/RiftExchange.json artifacts/
cp ../contracts/out/RiftExchange.sol/IERC20.json artifacts/
cp ../contracts/out/MockUSDT.sol/MockUSDT.json artifacts/
cp ../contracts/out/ERC1967Proxy.sol/ERC1967Proxy.json artifacts/
cp ../data-aggregation-contracts/out/BlockHeaderAggregator.sol/BlockHeaderAggregator.json artifacts/
cp ../data-aggregation-contracts/out/DepositVaultsAggregator.sol/DepositVaultsAggregator.json artifacts/
