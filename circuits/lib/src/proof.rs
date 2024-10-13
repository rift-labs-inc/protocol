use bitcoin::hashes::Hash;

use bitcoin::Block;

use crypto_bigint::{Encoding, U256};

use rift_core::btc_light_client::AsLittleEndianBytes;
use rift_core::lp::{compute_lp_hash, encode_liquidity_providers, LiquidityReservation};

use crate::transaction::{get_chainworks, serialize_no_segwit};
use crate::{generate_merkle_proof_and_root, AsRiftOptimizedBlock};
use rift_core::{CircuitInput, CircuitPublicValues};

use sp1_sdk::{ExecutionReport, HashableKey, ProverClient, SP1Stdin};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const MAIN_ELF: &[u8] = include_bytes!("../../elf/riscv32im-succinct-zkvm-elf");

pub fn compute_circuit_hash() -> String {
    let client = ProverClient::new();
    let (_, vk) = client.setup(MAIN_ELF);
    vk.bytes32().trim_start_matches("0x").to_string()
}

pub fn build_transaction_proof_input(
    order_nonce: &[u8; 32],
    liquidity_reservations: &Vec<LiquidityReservation>,
    safe_chainwork: U256,
    safe_block_height: u64,
    blocks: &[Block],
    proposed_block_index: usize,
    proposed_txid: &[u8; 32],
    retarget_block: &Block,
    retarget_block_height: u64,
) -> CircuitInput {
    let proposed_block = &blocks[proposed_block_index];

    let rift_optimized_blocks = &blocks
        .iter()
        .zip(safe_block_height..safe_block_height + blocks.len() as u64)
        .map(|(block, height)| block.as_rift_optimized_block(height))
        .collect::<Vec<_>>();

    let chainworks = get_chainworks(rift_optimized_blocks, safe_chainwork)
        .iter()
        .map(|x| x.to_be_bytes())
        .collect();

    let proposed_transaction = proposed_block
        .txdata
        .iter()
        .find(|tx| tx.compute_txid().to_byte_array().to_little_endian() == *proposed_txid);

    assert!(
        proposed_transaction.is_some(),
        "Mined transaction not found in the block"
    );
    let proposed_transaction = proposed_transaction.unwrap();
    let mined_transaction_serialized_no_segwit = serialize_no_segwit(proposed_transaction);

    let (merkle_proof, calculated_merkle_root) = generate_merkle_proof_and_root(
        proposed_block
            .txdata
            .iter()
            .map(|tx| {
                tx.compute_txid()
                    .as_raw_hash()
                    .as_byte_array()
                    .to_little_endian()
            })
            .collect(),
        *proposed_txid,
    );

    assert_eq!(
        calculated_merkle_root,
        proposed_block
            .compute_merkle_root()
            .unwrap()
            .to_byte_array()
            .to_little_endian(),
        "Invalid merkle root"
    );

    let lp_reservation_data_encoded = encode_liquidity_providers(liquidity_reservations);

    let safe_block_height = blocks.first().unwrap().bip34_block_height().unwrap();

    CircuitInput::new(
        CircuitPublicValues::new(
            proposed_transaction
                .compute_txid()
                .to_byte_array()
                .to_little_endian(),
            proposed_block
                .header
                .merkle_root
                .to_byte_array()
                .to_little_endian(),
            compute_lp_hash(
                &lp_reservation_data_encoded,
                liquidity_reservations.len() as u32,
            ),
            *order_nonce,
            liquidity_reservations.len() as u64,
            retarget_block
                .header
                .block_hash()
                .to_byte_array()
                .to_little_endian(),
            safe_block_height,
            proposed_block_index as u64,
            blocks.len() as u64 - 1 - proposed_block_index as u64,
            blocks
                .iter()
                .map(|block| block.header.block_hash().to_byte_array().to_little_endian())
                .collect(),
            chainworks,
            true,
        ),
        mined_transaction_serialized_no_segwit,
        merkle_proof,
        lp_reservation_data_encoded.to_vec(),
        rift_optimized_blocks.to_vec(),
        retarget_block.as_rift_optimized_block(retarget_block_height),
    )
}

pub fn build_block_proof_input(
    safe_chainwork: U256,
    safe_block_height: u64,
    blocks: &[Block],
    retarget_block: &Block,
    retarget_block_height: u64,
) -> CircuitInput {
    let rift_optimized_blocks = &blocks
        .iter()
        .zip(safe_block_height..safe_block_height + blocks.len() as u64)
        .map(|(block, height)| block.as_rift_optimized_block(height))
        .collect::<Vec<_>>();

    let chainworks = get_chainworks(rift_optimized_blocks, safe_chainwork)
        .iter()
        .map(|x| x.to_be_bytes())
        .collect();

    CircuitInput::new(
        CircuitPublicValues::new(
            [0u8; 32],
            [0u8; 32],
            [0u8; 32],
            [0u8; 32],
            0,
            retarget_block
                .header
                .block_hash()
                .to_byte_array()
                .to_little_endian(),
            safe_block_height,
            blocks.len() as u64 - 2,
            1,
            blocks
                .iter()
                .map(|block| block.header.block_hash().to_byte_array().to_little_endian())
                .collect(),
            chainworks,
            false,
        ),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        rift_optimized_blocks.to_vec(),
        retarget_block.as_rift_optimized_block(retarget_block_height),
    )
}

pub fn generate_plonk_proof(
    circuit_input: CircuitInput,
    verify: Option<bool>,
) -> sp1_sdk::SP1ProofWithPublicValues {
    // Setup the prover client.
    let client = ProverClient::new();
    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    stdin.write(&circuit_input);
    // Setup the program for proving.
    let (pk, vk) = client.setup(MAIN_ELF);
    // Generate the proof
    let proof = client
        .prove(&pk, stdin)
        .plonk()
        .run()
        .expect("failed to generate proof");

    // Verify the proof.
    if verify.unwrap_or(true) {
        client.verify(&proof, &vk).expect("failed to verify proof");
    }

    proof
}

pub fn execute(circuit_input: CircuitInput) -> (String, ExecutionReport) {
    let client = ProverClient::new();
    let mut stdin = SP1Stdin::new();
    stdin.write(&circuit_input);
    let (public_values, report) = client.execute(MAIN_ELF, stdin).run().unwrap();
    (public_values.raw(), report)
}
