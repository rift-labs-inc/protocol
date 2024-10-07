use crypto_bigint::CheckedAdd;
use crypto_bigint::U256;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub trait AsLittleEndianBytes {
    fn to_little_endian(self) -> Self;
}

impl<const N: usize> AsLittleEndianBytes for [u8; N] {
    fn to_little_endian(mut self) -> Self {
        self.reverse();
        self
    }
}

#[derive(Default, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Block {
    pub height: u64,
    pub version: [u8; 4],
    pub prev_blockhash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub time: [u8; 4],
    pub bits: [u8; 4],
    pub nonce: [u8; 4],
}

impl Block {
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(80);
        bytes.extend_from_slice(&self.version);
        bytes.extend_from_slice(&self.prev_blockhash);
        bytes.extend_from_slice(&self.merkle_root);
        bytes.extend_from_slice(&self.time);
        bytes.extend_from_slice(&self.bits);
        bytes.extend_from_slice(&self.nonce);

        assert_eq!(bytes.len(), 80, "Header must be exactly 80 bytes");
        bytes
    }

    pub fn compute_block_hash(&self) -> [u8; 32] {
        let header = self.serialize();
        let first_hash = Sha256::digest(header);
        let second_hash: [u8; 32] = Sha256::digest(first_hash).into();
        second_hash.to_little_endian()
    }

    pub fn compute_chainwork(&self, previous_block_chainwork: U256) -> U256 {
        previous_block_chainwork
            .checked_add(
                &U256::MAX
                    .checked_div(&bits_to_target(self.bits).checked_add(&U256::ONE).unwrap())
                    .unwrap(),
            )
            .unwrap()
    }
}

// taken from rust-bitcoin
pub fn bits_to_target(bits: [u8; 4]) -> U256 {
    let bits = u32::from_le_bytes(bits);
    let (mant, expt) = {
        let unshifted_expt = bits >> 24;
        if unshifted_expt <= 3 {
            ((bits & 0xFFFFFF) >> (8 * (3 - unshifted_expt as usize)), 0)
        } else {
            (bits & 0xFFFFFF, 8 * ((bits >> 24) - 3))
        }
    };
    if mant > 0x7F_FFFF {
        U256::ZERO
    } else {
        U256::from(mant) << expt as usize
    }
}

pub fn assert_pow(proposed_block_hash: &[u8; 32], proposed_block: &Block, proposed_target: U256) {
    let calculated_block_hash = proposed_block.compute_block_hash();

    // [2] verify proposed block hash matches calculated block hash
    assert_eq!(calculated_block_hash, *proposed_block_hash);

    // [3] verify PoW -> block hash <= proposed target
    assert!(
        U256::from_be_slice(proposed_block_hash).le(&proposed_target),
        "PoW invalid hash < target"
    );
}

pub fn verify_block(
    proposed_block_hash: [u8; 32],
    previous_block_hash: [u8; 32],
    proposed_block: &Block,
    retarget_block: &Block,
    previous_block_height: u64,
) {
    // [1] verify proposed target is equal to real target
    let proposed_target = bits_to_target(proposed_block.bits);

    assert_eq!(
        retarget_block.bits, proposed_block.bits,
        "Proposed target does not match real target"
    );

    // [2] verify the proposed block height is one greater than previous_block_height
    assert_eq!(
        proposed_block.height,
        previous_block_height + 1,
        "Block height is not one greater than previous block height"
    );

    // [3] verify the proposed prev_block_hash matches real previous_block_hash
    assert_eq!(
        proposed_block.prev_blockhash.to_little_endian(),
        previous_block_hash,
        "Proposed prev_block hash does not match real prev_block hash"
    );

    // [4] verify PoW (double sha256(block_hash) <= target)
    assert_pow(&proposed_block_hash, proposed_block, proposed_target);
}

pub fn assert_blockchain(
    commited_block_hashes: Vec<[u8; 32]>,
    commited_block_chainworks: Vec<U256>,
    safe_block_height: u64,
    retarget_block_hash: [u8; 32],
    blocks: Vec<Block>,
    retarget_block: Block,
) {
    assert_eq!(
        retarget_block.compute_block_hash(),
        retarget_block_hash,
        "Initial Retarget block hash mismatch"
    );

    assert_eq!(
        commited_block_hashes.len(),
        blocks.len(),
        "Block count mismatch between commited block hashes and blocks provided"
    );

    let mut current_chainwork = *commited_block_chainworks.first().unwrap();
    let mut last_retarget_block = retarget_block;
    // the first block in this array is a safe block aka known to the contract
    for i in 0..blocks.len() - 1 {
        let current_block = &blocks[i];
        let next_block = &blocks[i + 1];
        let current_block_hash = current_block.compute_block_hash();
        let next_block_hash = next_block.compute_block_hash();
        assert_eq!(
            current_block_hash, commited_block_hashes[i],
            "Commited block hash mismatch"
        );
        assert_eq!(
            next_block_hash,
            commited_block_hashes[i + 1],
            "Commited block hash mismatch"
        );

        // Change retarget block if necessary
        if next_block.height % 2016 == 0 {
            last_retarget_block = *next_block;
        }

        // assert chainwork is equal to commited chainwork
        assert_eq!(
            current_chainwork, commited_block_chainworks[i],
            "Chainwork mismatch"
        );

        // Update chainwork
        current_chainwork = next_block.compute_chainwork(current_chainwork);

        verify_block(
            next_block_hash,
            current_block_hash,
            next_block,
            &last_retarget_block,
            safe_block_height + i as u64,
        );
    }

    assert_eq!(
        current_chainwork,
        *commited_block_chainworks.last().unwrap(),
        "Chainwork mismatch"
    );

    assert_eq!(
        blocks.last().unwrap().compute_block_hash(),
        *commited_block_hashes.last().unwrap(),
        "Commited block hash mismatch"
    );
}
