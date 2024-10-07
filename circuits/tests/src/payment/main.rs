#[cfg(test)]
mod tests {
    use bitcoin::consensus::encode::deserialize;

    use bitcoin::hashes::Hash;

    use bitcoin::Block;

    use hex_literal::hex;

    use rift_core::btc_light_client::AsLittleEndianBytes;
    use rift_core::lp::{encode_liquidity_providers, LiquidityReservation};
    use rift_core::payment::{assert_bitcoin_payment, compint_to_u64};
    use rift_lib::transaction::{
        build_rift_payment_transaction, serialize_no_segwit, P2WPKHBitcoinWallet,
    };
    use rift_lib::{load_hex_bytes, to_hex_string};

    fn get_test_wallet() -> P2WPKHBitcoinWallet {
        P2WPKHBitcoinWallet::from_secret_key(
            hex!("ef7a6f48e45fc4af1ddfc9047af0e06f550bca661869455d5fc05812ef1a9593"),
            bitcoin::Network::Bitcoin,
        )
    }

    #[test]
    fn test_compint() {
        assert!(0x01 == compint_to_u64([0x01_u8]));

        assert!(0xFC == compint_to_u64([0xFC_u8]));

        assert!(0xF1 == compint_to_u64([0xF1_u8]));

        assert!(0xCDAB == compint_to_u64([0xFD, 0xAB, 0xCD]));

        assert!(0x01EFCDAB == compint_to_u64([0xFE, 0xAB, 0xCD, 0xEF, 0x01, 0x00]));

        assert!(
            0x01EFCDAB01EFCDAB
                == compint_to_u64([0xFF, 0xAB, 0xCD, 0xEF, 0x01, 0xAB, 0xCD, 0xEF, 0x01]),
        );

        assert!(99999 == compint_to_u64([0xFE, 0x9f, 0x86, 0x01, 0x00]));
    }

    #[test]
    fn assert_theo_btc_payment() {
        let wallet = get_test_wallet();

        let order_nonce = hex!("f0ad57e677a89d2c2aaae4c5fd52ba20c63c0a05c916619277af96435f874c64");
        let lp_reservations: Vec<LiquidityReservation> = vec![
            LiquidityReservation {
                expected_sats: 1000,
                script_pub_key: hex!("001463dff5f8da08ca226ba01f59722c62ad9b9b3eaa"),
            },
            LiquidityReservation {
                expected_sats: 2000,
                script_pub_key: hex!("0014aa86191235be8883693452cf30daf854035b085b"),
            },
            LiquidityReservation {
                expected_sats: 3000,
                script_pub_key: hex!("00146ab8f6c80b8a7dc1b90f7deb80e9b59ae16b7a5a"),
            },
        ];

        let utilized_block_height = 854136;
        let utilized_block_hash =
            hex!("00000000000000000003679bc829350e7b26cc98d54030c2edc5e470560c1fdc");
        let utilized_txid =
            hex!("8df99d697780166f12df68b1e2410a909374b6414da57a1a65f3b84eb8a4dd0f");
        let txvout = 4;
        let block = deserialize::<Block>(&load_hex_bytes(
            format!("data/block_{utilized_block_height}.hex").as_str(),
        ))
        .unwrap();

        assert_eq!(
            block.header.block_hash().to_byte_array().to_little_endian(),
            utilized_block_hash
        );
        let utilized_transaction = block
            .txdata
            .iter()
            .find(|tx| tx.compute_txid().to_byte_array().to_little_endian() == utilized_txid);
        assert!(
            utilized_transaction.is_some(),
            "Proposed transaction not found in the block"
        );

        let utilized_transaction = utilized_transaction.unwrap();

        assert_eq!(
            utilized_transaction
                .compute_txid()
                .to_byte_array()
                .to_little_endian(),
            utilized_txid,
            "Proposed transaction ID does not match the utilized transaction ID",
        );

        let unbroadcast_txn = build_rift_payment_transaction(
            order_nonce,
            &lp_reservations,
            utilized_txid,
            utilized_transaction,
            txvout,
            &wallet,
            1100,
        );

        let txn_data_no_segwit = serialize_no_segwit(&unbroadcast_txn);
        println!(
            "Unbroadcast txn: {:?}",
            to_hex_string(txn_data_no_segwit.as_slice())
        );

        assert_bitcoin_payment(
            txn_data_no_segwit.as_slice(),
            encode_liquidity_providers(&lp_reservations).to_vec(),
            order_nonce,
            lp_reservations.len() as u64,
        )
    }
}
