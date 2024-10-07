use bitcoin::consensus::encode::deserialize;

use bitcoin::Block;

use crypto_bigint::{Encoding, U256};
use hex_literal::hex;

use rift_core::lp::LiquidityReservation;

use rift_core::CircuitInput;
use rift_lib::proof::{self, build_transaction_proof_input};
use rift_lib::{get_retarget_height_from_block_height, load_hex_bytes, to_hex_string};

use clap::Parser;
use sp1_sdk::{ProverClient, SP1Stdin};

fn get_test_case_circuit_input() -> CircuitInput {
    let safe_chainwork = U256::from_be_bytes(hex!(
        "000000000000000000000000000000000000000085ed2ff0a553f14e4d649ce0"
    ));
    let order_nonce = hex!("f0ad57e677a89d2c2aaae4c5fd52ba20c63c0a05c916619277af96435f874c64");
    let lp_reservations: Vec<LiquidityReservation> = vec![
        LiquidityReservation {
            expected_sats: 487,
            script_pub_key: hex!("001463dff5f8da08ca226ba01f59722c62ad9b9b3eaa"),
        },
        LiquidityReservation {
            expected_sats: 487,
            script_pub_key: hex!("0014aa86191235be8883693452cf30daf854035b085b"),
        },
        LiquidityReservation {
            expected_sats: 487,
            script_pub_key: hex!("00146ab8f6c80b8a7dc1b90f7deb80e9b59ae16b7a5a"),
        },
    ];

    let mined_blocks = [
        deserialize::<Block>(&load_hex_bytes("tests/data/block_854373.hex")).unwrap(),
        deserialize::<Block>(&load_hex_bytes("tests/data/block_854374.hex")).unwrap(),
        deserialize::<Block>(&load_hex_bytes("tests/data/block_854375.hex")).unwrap(),
        deserialize::<Block>(&load_hex_bytes("tests/data/block_854376.hex")).unwrap(),
        deserialize::<Block>(&load_hex_bytes("tests/data/block_854377.hex")).unwrap(),
        deserialize::<Block>(&load_hex_bytes("tests/data/block_854378.hex")).unwrap(),
        deserialize::<Block>(&load_hex_bytes("tests/data/block_854379.hex")).unwrap(),
    ];

    let mined_block_height = 854374;
    let mined_txid = hex!("fb7ea6c1a58f9e827c50aefb3117ce41dd5fecb969041864ec0eff9273b08038");
    let retarget_block_height = get_retarget_height_from_block_height(mined_block_height);
    let mined_retarget_block = deserialize::<Block>(&load_hex_bytes(
        format!("tests/data/block_{retarget_block_height}.hex").as_str(),
    ))
    .unwrap();

    build_transaction_proof_input(
        &order_nonce,
        &lp_reservations,
        safe_chainwork,
        mined_blocks.first().unwrap().bip34_block_height().unwrap(),
        &mined_blocks,
        1,
        &mined_txid,
        &mined_retarget_block,
        mined_retarget_block.bip34_block_height().unwrap(),
    )
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run only the execute block without proof generation
    #[arg(long)]
    execute: bool,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Parse command line arguments
    let args = Args::parse();

    let circuit_input = get_test_case_circuit_input();

    println!("Circuit input generated successfully.");

    // Setup the prover client.
    let client = ProverClient::new();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    stdin.write(&circuit_input);
    println!("Inputs serialized successfully.");

    println!(
        "lp_reservation_hash: {:?}",
        to_hex_string(
            circuit_input
                .public_values
                .lp_reservation_hash
                .to_vec()
                .as_slice()
        )
    );

    if args.execute {
        // Execute the program
        let (_output, report) = client.execute(proof::MAIN_ELF, stdin).run().unwrap();
        println!("Program executed successfully.");
        println!("Number of cycles: {}", report.total_instruction_count());
    } else {
        // Setup the program for proving.
        let (pk, vk) = client.setup(proof::MAIN_ELF);

        // Generate the proof
        let proof = client
            .prove(&pk, stdin)
            .plonk()
            .run()
            .expect("failed to generate proof");

        println!("Successfully generated proof!");

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
        println!(
            "Public Inputs: {:?}",
            to_hex_string(proof.public_values.to_vec().as_slice())
        );
        println!("Solidity Ready Proof: {:?}", to_hex_string(&proof.bytes()));
    }
}
