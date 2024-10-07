use crate::constants::MAX_LIQUIDITY_PROVIDERS;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy)]
pub struct LiquidityReservation {
    pub expected_sats: u64,
    pub script_pub_key: [u8; 22],
}

pub fn build_hashable_chunk(lp_data: [[u8; 32]; 2], intermediate_vault_hash: [u8; 32]) -> [u8; 96] {
    let mut solidity_encoded_lp_data = [0u8; 96];
    solidity_encoded_lp_data[0..64].copy_from_slice(&lp_data[0..2].concat());
    // Copy the last 32-byte chunk from the intermediate vault hash
    solidity_encoded_lp_data[64..].copy_from_slice(&intermediate_vault_hash);
    solidity_encoded_lp_data
}

pub fn decode_liqudity_providers(
    liquidity_providers_encoded: Vec<[[u8; 32]; 2]>,
) -> [LiquidityReservation; MAX_LIQUIDITY_PROVIDERS] {
    let mut liquidity_providers = [LiquidityReservation {
        expected_sats: 0,
        script_pub_key: [0; 22],
    }; MAX_LIQUIDITY_PROVIDERS];

    for i in 0..MAX_LIQUIDITY_PROVIDERS {
        // Extract sats expected
        liquidity_providers[i].expected_sats = u64::from_be_bytes(
            liquidity_providers_encoded[i][0][32 - 8..]
                .try_into()
                .unwrap(),
        );

        // Extract script pub key
        liquidity_providers[i]
            .script_pub_key
            .copy_from_slice(&liquidity_providers_encoded[i][1][0..22]);
    }

    liquidity_providers
}

pub fn encode_liquidity_providers(
    liquidity_providers: &[LiquidityReservation],
) -> [[[u8; 32]; 2]; MAX_LIQUIDITY_PROVIDERS] {
    assert!(
        liquidity_providers.len() <= MAX_LIQUIDITY_PROVIDERS,
        "Too many liquidity providers"
    );
    let mut liquidity_providers_encoded = [[[0u8; 32]; 2]; MAX_LIQUIDITY_PROVIDERS];

    for i in 0..liquidity_providers.len() {
        // Encode sats expected
        liquidity_providers_encoded[i][0][32 - 8..]
            .copy_from_slice(&liquidity_providers[i].expected_sats.to_be_bytes());

        // Encode script pub key
        liquidity_providers_encoded[i][1][0..22]
            .copy_from_slice(&liquidity_providers[i].script_pub_key);
    }

    liquidity_providers_encoded[liquidity_providers.len()..]
        .iter_mut()
        .for_each(|lp| *lp = [[0u8; 32]; 2]);

    liquidity_providers_encoded
}

pub fn compute_lp_hash(lp_reservation_data_encoded: &[[[u8; 32]; 2]], lp_count: u32) -> [u8; 32] {
    assert!(
        lp_reservation_data_encoded.len() <= MAX_LIQUIDITY_PROVIDERS,
        "Too many liquidity providers"
    );
    let mut intermediate_vault_hash = [0u8; 32];

    for lp_data in lp_reservation_data_encoded.iter().take(lp_count as usize) {
        let hashable_chunk = build_hashable_chunk(*lp_data, intermediate_vault_hash);
        intermediate_vault_hash = Sha256::digest(hashable_chunk).into();
    }

    intermediate_vault_hash
}

pub fn assert_lp_hash(
    lp_reservation_hash: [u8; 32],
    lp_reservation_data_encoded: &[[[u8; 32]; 2]],
    lp_count: u32,
) {
    assert!(
        lp_reservation_data_encoded.len() <= MAX_LIQUIDITY_PROVIDERS,
        "Too many liquidity providers"
    );
    assert_eq!(
        compute_lp_hash(lp_reservation_data_encoded, lp_count),
        lp_reservation_hash,
        "Invalid LP hash"
    );
}
