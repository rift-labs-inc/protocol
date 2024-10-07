# E2E Tests 
## Prerequisites
- [Rust](https://www.rust-lang.org/tools/install)
- [Docker](https://docs.docker.com/get-docker/)
- [Bitcoin Core](https://bitcoin.org/en/download)
- [Foundry](https://getfoundry.sh)

## Run Devnet
```bash
RUST_LOG=main=debug,hypernode=debug,test_utils=debug cargo run --release --bin devnet
```

## Run E2E Tests
```bash
RUST_LOG=main=debug,hypernode=debug,test_utils=debug cargo test --release --test main -- --show-output
```
