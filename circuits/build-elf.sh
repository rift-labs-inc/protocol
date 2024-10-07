cargo test -p tests
cd program/
cargo prove build --docker --tag v2.0.0
cd ..
