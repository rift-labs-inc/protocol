pub mod proof;
pub mod transaction;

mod errors;
use bitcoin::hashes::hex::FromHex;
use bitcoin::hashes::Hash;
use std::fmt::Write;

use rift_core::btc_light_client::Block as RiftOptimizedBlock;
use rift_core::sha256_merkle::{hash_pairs, MerkleProofStep};

pub fn load_hex_bytes(file: &str) -> Vec<u8> {
    let hex_string = std::fs::read_to_string(file).expect("Failed to read file");
    Vec::<u8>::from_hex(&hex_string).expect("Failed to parse hex")
}

pub fn to_hex_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(&mut s, "{:02x}", b).unwrap();
    }
    s
}

pub fn get_retarget_height_from_block_height(block_height: u64) -> u64 {
    block_height - (block_height % 2016)
}

// Expects leaves to be in little-endian format (as shown on explorers)
pub fn generate_merkle_proof_and_root(
    leaves: Vec<[u8; 32]>,
    desired_leaf: [u8; 32],
) -> (Vec<MerkleProofStep>, [u8; 32]) {
    let mut current_level = leaves;
    let mut proof: Vec<MerkleProofStep> = Vec::new();
    let mut desired_index = current_level
        .iter()
        .position(|&leaf| leaf == desired_leaf)
        .expect("Desired leaf not found in the list of leaves");

    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        let mut i = 0;

        while i < current_level.len() {
            let left = current_level[i];
            let right = if i + 1 < current_level.len() {
                current_level[i + 1]
            } else {
                left
            };

            let parent_hash = hash_pairs(left, right);
            next_level.push(parent_hash);

            if i == desired_index || i + 1 == desired_index {
                let proof_step = if i == desired_index {
                    MerkleProofStep {
                        hash: right,
                        direction: true,
                    }
                } else {
                    MerkleProofStep {
                        hash: left,
                        direction: false,
                    }
                };
                proof.push(proof_step);
                desired_index /= 2;
            }

            i += 2;
        }

        current_level = next_level;
    }

    let merkle_root = current_level[0];
    (proof, merkle_root)
}

pub trait AsRiftOptimizedBlock {
    fn as_rift_optimized_block(&self, height: u64) -> RiftOptimizedBlock;
    fn as_rift_optimized_block_unsafe(&self) -> RiftOptimizedBlock;
}

impl AsRiftOptimizedBlock for bitcoin::Block {
    fn as_rift_optimized_block(&self, height: u64) -> RiftOptimizedBlock {
        RiftOptimizedBlock {
            height,
            version: self.header.version.to_consensus().to_le_bytes(),
            prev_blockhash: self.header.prev_blockhash.to_raw_hash().to_byte_array(),
            merkle_root: self.header.merkle_root.to_raw_hash().to_byte_array(),
            time: self.header.time.to_le_bytes(),
            bits: self.header.bits.to_consensus().to_le_bytes(),
            nonce: self.header.nonce.to_le_bytes(),
        }
    }

    fn as_rift_optimized_block_unsafe(&self) -> RiftOptimizedBlock {
        RiftOptimizedBlock {
            height: self.bip34_block_height().unwrap(),
            version: self.header.version.to_consensus().to_le_bytes(),
            prev_blockhash: self.header.prev_blockhash.to_raw_hash().to_byte_array(),
            merkle_root: self.header.merkle_root.to_raw_hash().to_byte_array(),
            time: self.header.time.to_le_bytes(),
            bits: self.header.bits.to_consensus().to_le_bytes(),
            nonce: self.header.nonce.to_le_bytes(),
        }
    }
}
