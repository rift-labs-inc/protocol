# circuits 

## Requirements

- [Rust](https://rustup.rs/)
- [SP1](https://succinctlabs.github.io/sp1/getting-started/install.html)
- [Docker](https://docs.docker.com/get-docker/)

### Directory Overview

| Directory | Purpose | Contents |
|-----------|---------|----------|
| `core/`    | Internal Library | Encapsulates all circuit business logic |
| `program/`| Executable Wrapper | Combines SP1 with our circuit library to create the program executable |
| `script/` | Build Utilities | Contains scripts for building vkeys, proofs and evm artifacts |
| `lib/`  | Client Library | Client-facing library for creating proofs|
| `tests/`  | Testing Suite | Unit and Integration tests |


### Run Unit Tests
```sh
./download_test_blocks.sh
cargo test -p tests
```

### Run Specific Test
```sh
cargo test -p tests --test <test_name>
# <tx_hash | sha256_merkle | bitcoin | lp_hash | payment | giga>
```

### Build Demo Mainnet Plonk Proof

#### Execute only
```
cargo run --release --bin plonk_test -- --execute
```

#### Proof Gen
```sh
cargo run --release --bin plonk_test
```

### Build SP1 Program
```sh
cd program && cargo prove build && cd ..
```

### Compute the Verification Key

```sh
cargo run --release --bin vkey
```
