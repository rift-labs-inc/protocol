use sha2::{Digest, Sha256};

pub fn sha256_hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn get_natural_txid(tx: &[u8]) -> [u8; 32] {
    let intermediate_hash = sha256_hash(tx);
    sha256_hash(&intermediate_hash)
}
