#[cfg(test)]
mod tests {
    use bitcoin::consensus::encode::deserialize;

    use bitcoin::hashes::Hash;

    use bitcoin::Block;

    use crypto_bigint::Encoding;
    use crypto_bigint::U256;
    use hex_literal::hex;

    use rift_core::lp::{compute_lp_hash, encode_liquidity_providers, LiquidityReservation};

    use rift_core::btc_light_client::AsLittleEndianBytes;
    use rift_core::{validate_rift_transaction, CircuitInput, CircuitPublicValues};
    use rift_lib::transaction::serialize_no_segwit;
    use rift_lib::{
        generate_merkle_proof_and_root, get_retarget_height_from_block_height, load_hex_bytes,
        AsRiftOptimizedBlock,
    };

    fn get_test_case_circuit_input() -> CircuitInput {
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

        let lp_reservation_data_encoded = encode_liquidity_providers(&lp_reservations);
        let safe_chainwork = U256::from_be_bytes(hex!(
            "000000000000000000000000000000000000000085ed2ff0a553f14e4d649ce0"
        ));

        let mined_blocks = [
            deserialize::<Block>(&load_hex_bytes("data/block_854373.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854374.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854375.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854376.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854377.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854378.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854379.hex")).unwrap(),
        ];

        let chainworks: Vec<_> = mined_blocks
            .iter()
            .map(|block| block.as_rift_optimized_block_unsafe())
            .scan(safe_chainwork, |chainwork_acc, block| {
                *chainwork_acc = block.compute_chainwork(*chainwork_acc);
                Some(*chainwork_acc)
            })
            .map(|chainwork| chainwork.to_be_bytes())
            .collect();

        let mined_block_height = 854374;
        let mined_block = deserialize::<Block>(&load_hex_bytes(
            format!("data/block_{mined_block_height}.hex").as_str(),
        ))
        .unwrap();
        let mined_txid = hex!("fb7ea6c1a58f9e827c50aefb3117ce41dd5fecb969041864ec0eff9273b08038");
        let retarget_block_height = get_retarget_height_from_block_height(mined_block_height);
        let mined_retarget_block = deserialize::<Block>(&load_hex_bytes(
            format!("data/block_{retarget_block_height}.hex").as_str(),
        ))
        .unwrap();

        // This is a real mainnet transaction so we don't need to build rift payment transaction

        let mined_transaction = mined_block
            .txdata
            .iter()
            .find(|tx| tx.compute_txid().to_byte_array().to_little_endian() == mined_txid);

        assert!(
            mined_transaction.is_some(),
            "Mined transaction not found in the block"
        );
        let mined_transaction = mined_transaction.unwrap();
        let mined_transaction_serialized_no_segwit = serialize_no_segwit(mined_transaction);

        let txn = mined_transaction
            .compute_txid()
            .as_byte_array()
            .to_little_endian();

        let (merkle_proof, calculated_merkle_root) = generate_merkle_proof_and_root(
            mined_block
                .txdata
                .iter()
                .map(|tx| {
                    tx.compute_txid()
                        .as_raw_hash()
                        .as_byte_array()
                        .to_little_endian()
                })
                .collect(),
            txn,
        );

        assert_eq!(
            calculated_merkle_root,
            mined_block
                .compute_merkle_root()
                .unwrap()
                .to_byte_array()
                .to_little_endian(),
            "Invalid merkle root"
        );
        println!("Merkle proof generated successfully.");

        CircuitInput::new(
            CircuitPublicValues::new(
                mined_transaction
                    .compute_txid()
                    .to_byte_array()
                    .to_little_endian(),
                mined_block
                    .header
                    .merkle_root
                    .to_byte_array()
                    .to_little_endian(),
                compute_lp_hash(&lp_reservation_data_encoded, lp_reservations.len() as u32),
                order_nonce,
                lp_reservations.len() as u64,
                mined_retarget_block
                    .header
                    .block_hash()
                    .to_byte_array()
                    .to_little_endian(),
                mined_block_height - 1,
                1,
                5,
                mined_blocks
                    .iter()
                    .map(|block| block.header.block_hash().to_byte_array().to_little_endian())
                    .collect(),
                chainworks,
                true,
            ),
            mined_transaction_serialized_no_segwit,
            merkle_proof,
            lp_reservation_data_encoded.to_vec(),
            mined_blocks
                .iter()
                .map(|block| block.as_rift_optimized_block_unsafe())
                .collect(),
            mined_retarget_block.as_rift_optimized_block_unsafe(),
        )
    }

    #[test]
    fn test_mainnet_rift_txn() {
        validate_rift_transaction(get_test_case_circuit_input());
    }

    #[test]
    fn test_circuit_input_serialization_functional() {
        let circuit_input = get_test_case_circuit_input();

        println!("Circuit input generated successfully.");

        // Serialize the circuit input
        let serialized = bincode::serialize(&circuit_input).unwrap_or_else(|e| {
            panic!("Failed to serialize circuit input: {}", e);
        });
        println!("Serialized data size: {} bytes", serialized.len());

        // Deserialize back to CircuitInput
        let _deserialized: CircuitInput = bincode::deserialize(&serialized).unwrap();

        println!("Serialization and deserialization successful!");
        println!("Deserialization successful!");

        println!("Serialization test passed successfully!");
    }

    #[test]
    fn test_mainnet_block_proof() {
        use rift_lib::proof::build_block_proof_input;

        let safe_chainwork = U256::from_be_bytes(hex!(
            "000000000000000000000000000000000000000085ed2ff0a553f14e4d649ce0"
        ));

        let mined_blocks = [
            deserialize::<Block>(&load_hex_bytes("data/block_854373.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854374.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854375.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854376.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854377.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854378.hex")).unwrap(),
            deserialize::<Block>(&load_hex_bytes("data/block_854379.hex")).unwrap(),
        ];

        let safe_block_height = mined_blocks[0].bip34_block_height().unwrap();
        let retarget_block_height = get_retarget_height_from_block_height(safe_block_height);
        let retarget_block = deserialize::<Block>(&load_hex_bytes(
            format!("data/block_{retarget_block_height}.hex").as_str(),
        ))
        .unwrap();

        let circuit_input = build_block_proof_input(
            safe_chainwork,
            safe_block_height,
            &mined_blocks,
            &retarget_block,
            retarget_block_height,
        );

        // Verify that the circuit input is created correctly
        assert_eq!(
            circuit_input.public_values.safe_block_height,
            safe_block_height
        );
        assert_eq!(
            circuit_input.public_values.confirmation_block_height_delta,
            mined_blocks.len() as u64 - 1
        );
        assert_eq!(circuit_input.utilized_blocks, mined_blocks.len() as u64);

        // Verify that the retarget block is set correctly
        assert_eq!(
            circuit_input.retarget_block.compute_block_hash(),
            retarget_block
                .as_rift_optimized_block_unsafe()
                .compute_block_hash()
        );

        // Verify that the is_transaction_proof flag is set to false
        assert!(!circuit_input.public_values.is_transaction_proof);

        println!("Block proof input generated and verified successfully.");
    }
}
