# Rift Exchange Contracts

[Current Exchange Deployment](https://arbiscan.io/address/0xdc63082c8bfebc973f2906fbfdb696e88735cca3)

*Gas Golfers beware, highly unoptimized code ahead.*

## Dependencies

- [Foundry](https://github.com/foundry-rs/foundry)

### Installation

To install contract dependencies, run the following command:

```bash
forge soldeer install
```

For live network deployments, install OpenZeppelin upgrades helper library globally:

```bash
npm install -g @openzeppelin/upgrades-core@1.39.0
```

## Deployments

### Arbitrum Mainnet

#### Deploy Rift Exchange
```bash
source .env && forge clean && forge build --via-ir && \
forge script --chain arbitrum scripts/DeployRiftExchange.s.sol:DeployRiftExchange \
--rpc-url $ARBITRUM_RPC_URL --broadcast --sender $SENDER --private-key $SENDER_PRIVATE_KEY \
--verify --etherscan-api-key $ARBITRUM_ETHERSCAN_API_KEY --ffi -vvvv --via-ir
```

#### Upgrade Rift Exchange
```bash
source .env && forge clean && forge build --via-ir && \
forge script --chain arbitrum scripts/UpgradeRiftExchange.s.sol:UpgradeRiftExchange \
--rpc-url $ARBITRUM_RPC_URL --broadcast --sender $SENDER --private-key $SENDER_PRIVATE_KEY \
--verify --etherscan-api-key $ARBITRUM_ETHERSCAN_API_KEY --ffi -vvvv --via-ir
```

### Arbitrum Sepolia

#### Deploy Rift Exchange
```bash
npm i @openzeppelin/upgrades-core@1.39.0 -g
source .env && forge clean && forge build --via-ir && \
forge script --chain arbitrum-sepolia scripts/DeployRiftExchange.s.sol:DeployRiftExchange \
--rpc-url $ARBITRUM_SEPOLIA_RPC_URL --broadcast --sender $SENDER --private-key $SENDER_PRIVATE_KEY \
--verify --etherscan-api-key $ARBITRUM_ETHERSCAN_API_KEY --ffi -vvvv --via-ir
```

## Testing

### Unit Tests
```bash
forge test
```

### Static Analysis

#### Slither
1. Install [slither](https://github.com/crytic/slither)
2. Run:
   ```bash
   python -m slither .
   ```

#### Mythril
1. Install [mythril](https://github.com/ConsenSys/mythril)
2. Run:
   ```bash
   myth analyze src/RiftExchange.sol --solc-json mythril.config.json
   ```

### Invariants

- Invariant 1: Unreserved balance should never exceed initial balance
```pseudocode
map(reservation.initialBalance >= reservation.unreservedBalance)
```

Invariant 2: The sum of differences between initial balance and unreserved balance across all deposit vaults should equal the sum of all non-completed reserved amounts
(Time expired + Created reservation states)
```pseudocode
sum(depositVault.initialBalance - depositVault.unreservedBalance for all depositVaults) ==
    sum(nonCompletedReservations.amountsToReserve for all nonCompletedReservations)
```
