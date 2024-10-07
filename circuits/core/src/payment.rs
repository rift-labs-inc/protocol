use crate::{
    constants::MAX_LIQUIDITY_PROVIDERS,
    lp::{decode_liqudity_providers, LiquidityReservation},
};

// Constants
//const MAX_SCRIPTSIG_SIZE: u64 = 22;
const MAX_INPUT_COUNT: u64 = 1;
//const MAX_SCRIPT_INSCRPITION_SIZE: u64 = 80;
//const VERSION_LEN: u8 = 4;
const TXID_LEN: u8 = 32;
const VOUT_LEN: u8 = 4;
const SEQUENCE_LEN: u8 = 4;
const AMOUNT_LEN: u8 = 8;
const OP_RETURN_CODE: u8 = 0x6a;
const OP_PUSHBYTES_32: u8 = 0x20;
//const DATA_LEN: u8 = 80;

// Structs
/*
struct TxOut {
    value: u64,
    txout_script_length: u8,
    txout_script: [u8; MAX_SCRIPTSIG_SIZE as usize],
}
*/

// Helper functions
fn to_int<const N: usize>(bytes: [u8; N]) -> u64 {
    bytes.iter().fold(0u64, |acc, &b| (acc << 8) | b as u64)
}

pub fn compint_to_u64<const N: usize>(compact_bytes: [u8; N]) -> u64 {
    let start_byte = compact_bytes[0];
    match start_byte {
        0xFD => to_int(grab_bytes_le::<2>(&compact_bytes[1..])),
        0xFE => to_int(grab_bytes_le::<4>(&compact_bytes[1..])),
        0xFF => to_int(grab_bytes_le::<8>(&compact_bytes[1..])),
        _ => start_byte as u64,
    }
}

fn compint_start_to_byte_len(start_byte: u8) -> u8 {
    match start_byte {
        0xFD => 3,
        0xFE => 5,
        0xFF => 9,
        _ => 1,
    }
}

fn extract_int_from_compint_pointer(data_pointer: u64, txn_data: &[u8]) -> (u64, u8) {
    let counter_byte_len = compint_start_to_byte_len(txn_data[data_pointer as usize]);
    let counter = compint_to_u64(grab_bytes_be_conditional::<9>(
        txn_data,
        data_pointer,
        |i| i < counter_byte_len as u64,
    ));
    (counter, counter_byte_len)
}

fn assert_payment_utxos_exist(
    txn_data: &[u8],
    reserved_liquidity_providers: &[LiquidityReservation; MAX_LIQUIDITY_PROVIDERS],
    lp_count: u64,
    order_nonce: [u8; 32],
) {
    let mut data_pointer = 4;
    let (input_counter, input_counter_byte_len) =
        extract_int_from_compint_pointer(data_pointer, txn_data);
    data_pointer += input_counter_byte_len as u64;
    assert_eq!(input_counter, MAX_INPUT_COUNT);

    // Skip inputs
    for _ in 0..MAX_INPUT_COUNT {
        data_pointer += (TXID_LEN + VOUT_LEN) as u64;
        let (sig_counter, sig_counter_byte_len) =
            extract_int_from_compint_pointer(data_pointer, txn_data);
        data_pointer += sig_counter + sig_counter_byte_len as u64 + SEQUENCE_LEN as u64;
    }

    let (output_counter, output_counter_byte_len) =
        extract_int_from_compint_pointer(data_pointer, txn_data);
    assert!(output_counter <= MAX_LIQUIDITY_PROVIDERS as u64);
    assert!(lp_count < output_counter);
    data_pointer += output_counter_byte_len as u64;

    for (i, _lp) in reserved_liquidity_providers
        .iter()
        .enumerate()
        .take(MAX_LIQUIDITY_PROVIDERS)
    {
        if i < lp_count as usize {
            let value = to_int::<8>(grab_bytes_le::<8>(&txn_data[data_pointer as usize..]));
            data_pointer += AMOUNT_LEN as u64;
            let (sig_counter, sig_counter_byte_len) =
                extract_int_from_compint_pointer(data_pointer, txn_data);
            data_pointer += sig_counter_byte_len as u64;

            assert_eq!(sig_counter, 22);

            let locking_script =
                grab_bytes_be_conditional::<22>(txn_data, data_pointer, |i| i < sig_counter);

            let expected_sats = reserved_liquidity_providers[i].expected_sats;

            assert_eq!(value, expected_sats);

            assert_eq!(
                locking_script,
                reserved_liquidity_providers[i].script_pub_key
            );

            data_pointer += sig_counter;
        }
    }

    data_pointer += AMOUNT_LEN as u64;
    let (sig_counter, sig_counter_byte_len) =
        extract_int_from_compint_pointer(data_pointer, txn_data);
    data_pointer += sig_counter_byte_len as u64;

    assert_eq!(sig_counter, 34);

    assert_eq!(txn_data[data_pointer as usize], OP_RETURN_CODE);
    data_pointer += 1;
    assert_eq!(txn_data[data_pointer as usize], OP_PUSHBYTES_32);
    data_pointer += 1;

    let inscribed_order_nonce =
        grab_bytes_be_conditional::<32>(txn_data, data_pointer, |i| i < sig_counter);
    assert_eq!(inscribed_order_nonce, order_nonce);
}

pub fn assert_bitcoin_payment(
    txn_data_no_segwit: &[u8],
    lp_reservation_data_encoded: Vec<[[u8; 32]; 2]>,
    order_nonce: [u8; 32],
    lp_count: u64,
) {
    assert!(lp_reservation_data_encoded.len() <= MAX_LIQUIDITY_PROVIDERS);
    let liquidity_providers = decode_liqudity_providers(lp_reservation_data_encoded);
    assert_payment_utxos_exist(
        txn_data_no_segwit,
        &liquidity_providers,
        lp_count,
        order_nonce,
    );
}

// Helper functions (placeholders, implement as needed)
fn grab_bytes_le<const N: usize>(data: &[u8]) -> [u8; N] {
    let mut result = [0u8; N];
    result.copy_from_slice(&data[..N]);
    result.reverse();
    result
}

fn grab_bytes_be_conditional<const N: usize>(
    data: &[u8],
    start: u64,
    condition: impl Fn(u64) -> bool,
) -> [u8; N] {
    let mut result = [0u8; N];
    for i in 0..N {
        if condition(i as u64) {
            result[i] = data[start as usize + i];
        }
    }
    result
}
