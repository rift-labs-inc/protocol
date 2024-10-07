#[cfg(test)]
mod tests {
    use rift_core::lp::{assert_lp_hash, encode_liquidity_providers, LiquidityReservation};

    use hex_literal::hex;

    #[test]
    fn test_assert_lp_hash() {
        let liquidity_providers = vec![LiquidityReservation {
            expected_sats: 1230,
            script_pub_key: hex!("0014841b80d2cc75f5345c482af96294d04fdd66b2b7"),
        }];

        let encoded_lps = encode_liquidity_providers(&liquidity_providers);

        let expected_vault_hash: [u8; 32] =
            hex!("511b6e0b655b765a6407d8475eb61a9619bde367bebc51fdd5f93e6d5474ee4d");

        assert_lp_hash(expected_vault_hash, &encoded_lps, 1);
    }
}
