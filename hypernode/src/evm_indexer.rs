use alloy::network::eip2718::Encodable2718;
use alloy::primitives::FixedBytes;
use alloy::providers::WalletProvider;
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    network::TransactionBuilder,
    primitives::{Bytes, U256},
    providers::Provider,
    rpc::types::{BlockTransactionsKind, Filter, TransactionInput, TransactionRequest},
    sol_types::{SolEvent, SolValue},
};
use bitcoin::hex::DisplayHex;
use futures::stream::{self, TryStreamExt};
use futures_util::StreamExt;
use log::{error, info};
use std::time::Instant;
use std::{collections::HashMap, collections::HashSet, sync::Arc};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::core::{EvmHttpProvider, RiftExchangeWebsocket};
use crate::error::HypernodeError;
use crate::evm_block_trigger::{self, EvmBlockTrigger};
use crate::{
    constants::HEADER_LOOKBACK_LIMIT,
    core::{
        BlockHeaderAggregator, DepositVaultAggregator, ReservationMetadata,
        RiftExchange::{self},
        ThreadSafeStore,
    },
};
use crate::{hyper_err, Result};

pub async fn broadcast_transaction_via_flashbots(
    contract: &Arc<RiftExchangeWebsocket>,
    flashbots_provider: &EvmHttpProvider,
    txn_calldata: &[u8],
) -> Result<FixedBytes<32>> {
    let provider = contract.provider();
    let tx = TransactionRequest::default()
        .to(*contract.address())
        .input(TransactionInput::new(txn_calldata.to_vec().into()));

    let tx = provider
        .fill(tx)
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to fill transaction: {}", e))?;

    let tx_envelope = tx
        .as_builder()
        .unwrap()
        .clone()
        .build(&provider.wallet())
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to build transaction envelope: {}", e))?;

    let tx_encoded = tx_envelope.encoded_2718();
    let pending = flashbots_provider
        .send_raw_transaction(&tx_encoded)
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to send raw transaction: {}", e))?
        .register()
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to register transaction: {}", e))?;

    Ok(*pending.tx_hash())
}

pub async fn broadcast_transaction(
    contract: &Arc<RiftExchangeWebsocket>,
    txn_calldata: &[u8],
    debug_url: &str,
) -> Result<FixedBytes<32>> {
    let provider = contract.provider();
    let tx = TransactionRequest::default()
        .to(*contract.address())
        .input(TransactionInput::new(txn_calldata.to_vec().into()));

    match provider.send_transaction(tx.clone()).await {
        Ok(tx) => Ok(*tx.tx_hash()),
        Err(e) => {
            let block_height = provider
                .get_block_number()
                .await
                .map_err(|e| hyper_err!(Evm, "Failed to get block number: {}", e))?;

            let data = txn_calldata.as_hex();
            let to = contract.address().to_string();
            info!(
                    "To debug failed proof broadcast run: cast call {} --data {} --trace --block {} --rpc-url {}",
                    to,
                    data,
                    block_height,
                    debug_url
                );
            Err(hyper_err!(Evm, "Failed to broadcast proof: {}", e))
        }
    }
}

pub async fn fetch_token_decimals(exchange: &RiftExchangeWebsocket) -> Result<u8> {
    exchange
        .tokenDecimals()
        .call()
        .await
        .map(|v| v._0)
        .map_err(|e| hyper_err!(Evm, "Failed to fetch token decimals: {}", e))
}

fn decode_vaults(encoded_vaults: Vec<u8>) -> Result<Vec<RiftExchange::DepositVault>> {
    let encoded_vaults = Vec::<Bytes>::abi_decode(&encoded_vaults, false)
        .map_err(|_| hyper_err!(Decode, "Failed to decode vault byte array"))?;
    encoded_vaults
        .into_iter()
        .map(|bytes| {
            RiftExchange::DepositVault::abi_decode(&bytes.to_vec(), false)
                .map_err(|e| hyper_err!(Decode, "Failed to decode deposit vault: {}", e))
        })
        .collect()
}

pub async fn download_vaults(
    contract: Arc<RiftExchangeWebsocket>,
    vault_indices: Vec<u32>,
) -> Result<Vec<RiftExchange::DepositVault>> {
    let provider = contract.provider();
    let encoded_vaults = DepositVaultAggregator::deploy_builder(
        provider,
        vault_indices.clone(),
        *contract.address(),
    )
    .call()
    .await
    .map_err(|e| hyper_err!(Evm, "Failed to call DepositVaultAggregator: {}", e))?;

    decode_vaults(encoded_vaults.to_vec())
}

fn decode_block_hashes(encoded_blocks: Vec<u8>) -> Result<Vec<[u8; 32]>> {
    <Vec<Bytes>>::abi_decode(&encoded_blocks, false)
        .map_err(|e| hyper_err!(Decode, "Failed to decode block hashes: {}", e))?
        .into_iter()
        .map(|bytes| {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&bytes);
            Ok(hash)
        })
        .collect()
}

// get available bitcoin headers stored on the rift exchange contract
pub async fn download_safe_bitcoin_headers(
    contract: Arc<RiftExchangeWebsocket>,
    store: Arc<ThreadSafeStore>,
    end_block_height: Option<u64>,
    lookback_count: Option<usize>,
) -> Result<u64> {
    let provider = contract.provider();
    let current_evm_tip = provider
        .get_block_number()
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to get block number: {}", e))?;

    let stored_tip = match end_block_height {
        Some(height) => height,
        None => u64::from_be_bytes(
            contract
                .currentHeight()
                .call()
                .await
                .map_err(|e| hyper_err!(Evm, "Failed to get current height: {}", e))?
                ._0
                .to_be_bytes::<32>()[32 - 8..]
                .try_into()
                .map_err(|_| hyper_err!(Conversion, "Failed to convert current height to u64"))?,
        ),
    };

    let lookback_limit = lookback_count.unwrap_or(HEADER_LOOKBACK_LIMIT);

    let mut heights = (0..lookback_limit)
        .map(|i| U256::from(stored_tip).saturating_sub(U256::from(i)))
        .collect::<Vec<_>>();

    heights.dedup();

    let encoded_blocks =
        BlockHeaderAggregator::deploy_builder(provider, heights.clone(), *contract.address())
            .call()
            .await
            .map_err(|e| hyper_err!(Evm, "Failed to call BlockHeaderAggregator: {}", e))?;

    let block_hashes = decode_block_hashes(encoded_blocks.to_vec())?;

    store
        .with_lock(|store_guard| {
            for (i, hash) in block_hashes.iter().enumerate() {
                if hash == &[0u8; 32] {
                    continue;
                }
                store_guard.safe_contract_block_hashes.insert(
                    u64::from_be_bytes(
                        heights[i].to_be_bytes::<32>()[32 - 8..].try_into().unwrap(),
                    ),
                    *hash,
                );
            }
        })
        .await;

    Ok(current_evm_tip)
}

// Downloads the actual reservation struct and all utilized deposit vaults
pub async fn download_reservation(
    reservation_id: U256,
    contract: Arc<RiftExchangeWebsocket>,
) -> Result<(U256, ReservationMetadata)> {
    let reservation = contract
        .getReservation(reservation_id)
        .call()
        .await
        .map_err(|e| {
            hyper_err!(
                Evm,
                "Failed to get reservation with ID {:?}: {}",
                reservation_id,
                e
            )
        })?
        ._0;

    let vault_indexes: Vec<u32> = reservation
        .vaultIndexes
        .iter()
        .map(|index| u32::from_be_bytes(index.to_be_bytes::<32>()[32 - 4..].try_into().unwrap()))
        .collect();

    let vaults = download_vaults(contract, vault_indexes).await?;
    Ok((
        reservation_id,
        ReservationMetadata::new(reservation, vaults),
    ))
}

pub async fn find_block_height_from_time(
    rift_exchange: &RiftExchangeWebsocket,
    hours: u64,
    average_time_between_evm_blocks: u64,
) -> Result<u64> {
    let time = Instant::now();

    let provider = rift_exchange.provider();
    let current_block = provider
        .get_block(BlockId::latest(), BlockTransactionsKind::Hashes)
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to get latest block: {}", e))?
        .ok_or_else(|| hyper_err!(Evm, "Latest block not found"))?;
    let current_timestamp = &current_block.header.timestamp;
    let target_timestamp = current_timestamp.saturating_sub(hours * 3600);

    let blocks_per_hour = 3600 / average_time_between_evm_blocks;
    let estimated_blocks_ago = hours * blocks_per_hour;

    let mut check_block = current_block
        .header
        .number
        .unwrap()
        .saturating_sub(estimated_blocks_ago);

    loop {
        let block_info = provider
            .get_block(
                BlockId::Number(BlockNumberOrTag::Number(check_block)),
                BlockTransactionsKind::Hashes,
            )
            .await
            .map_err(|e| hyper_err!(Evm, "Failed to get block {}: {}", check_block, e))?
            .ok_or_else(|| hyper_err!(Evm, "Block {} not found", check_block))?;
        let block_timestamp = block_info.header.timestamp;

        if block_timestamp <= target_timestamp || check_block == 0 {
            info!(
                "Found EVM block height: {}, {:.2} hours from tip in {:?}",
                check_block,
                (current_timestamp - block_timestamp) as f64 / 3600 as f64,
                time.elapsed()
            );
            return Ok(check_block.into());
        }

        check_block = check_block.saturating_sub(blocks_per_hour);
    }
}

// Goes through the last RESERVATION_DURATION_HOURS worth of ethereum blocks and collects all reservations
pub async fn sync_reservations(
    contract: Arc<RiftExchangeWebsocket>,
    safe_store: Arc<ThreadSafeStore>,
    trigger: Arc<EvmBlockTrigger>,
    start_block: u64,
    rpc_concurrency: usize,
) -> Result<u64> {
    let time = Instant::now();
    info!("Syncing reservations from block {}", start_block);
    let provider = contract.provider();
    let latest_block: u64 = provider
        .get_block_number()
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to get latest block number: {}", e))?;
    let log_filter = Filter::new()
        .address(*contract.address())
        .from_block(start_block)
        .to_block(latest_block);

    let logs = provider
        .get_logs(&log_filter)
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to get logs: {}", e))?;

    let mut active_reservations_set: HashSet<U256> = HashSet::new();
    let mut reservations_in_challenge: HashSet<U256> = HashSet::new();

    for log in logs {
        match log.topic0() {
            Some(&RiftExchange::LiquidityReserved::SIGNATURE_HASH) => {
                let reservation_event: RiftExchange::LiquidityReserved = log
                    .log_decode()
                    .map_err(|e| {
                        hyper_err!(Decode, "Failed to decode LiquidityReserved event: {}", e)
                    })?
                    .inner
                    .data;
                active_reservations_set.insert(reservation_event.swapReservationIndex);
            }
            Some(&RiftExchange::ProofSubmitted::SIGNATURE_HASH) => {
                let proof_proposed_event: RiftExchange::ProofSubmitted = log
                    .log_decode()
                    .map_err(|e| {
                        hyper_err!(Decode, "Failed to decode ProofSubmitted event: {}", e)
                    })?
                    .inner
                    .data;
                active_reservations_set.remove(&proof_proposed_event.swapReservationIndex);
                reservations_in_challenge.insert(proof_proposed_event.swapReservationIndex);
            }
            Some(&RiftExchange::SwapComplete::SIGNATURE_HASH) => {
                let swap_complete_event: RiftExchange::SwapComplete = log
                    .log_decode()
                    .map_err(|e| hyper_err!(Decode, "Failed to decode SwapComplete event: {}", e))?
                    .inner
                    .data;
                active_reservations_set.remove(&swap_complete_event.swapReservationIndex);
                reservations_in_challenge.remove(&swap_complete_event.swapReservationIndex);
            }
            _ => {}
        }
    }

    let reservations_to_download: HashSet<U256> = active_reservations_set
        .union(&reservations_in_challenge)
        .cloned()
        .collect();

    let active_reservations_ids: Vec<U256> = active_reservations_set.into_iter().collect();

    let reservations_to_download: Vec<U256> = reservations_to_download.into_iter().collect();

    // download all reservation data from the contract, both active and in challenge
    let downloaded_reservations: HashMap<U256, ReservationMetadata> =
        stream::iter(reservations_to_download)
            .map(|reservation_id| {
                let contract: Arc<RiftExchangeWebsocket> = Arc::clone(&contract);
                async move {
                    download_reservation(reservation_id, contract)
                        .await
                        .map_err(|e| {
                            info!("Failed to download reservation: {}", e);
                            e
                        })
                }
            })
            .buffer_unordered(rpc_concurrency)
            .try_collect()
            .await
            .map_err(|e| hyper_err!(Evm, "Failed to download reservations: {}", e))?;

    let reservations = downloaded_reservations
        .clone()
        .into_iter()
        .filter(|(id, _)| active_reservations_ids.contains(id))
        .collect::<HashMap<U256, ReservationMetadata>>();

    let total_reservations = &reservations.len();

    // Update active_reservations
    safe_store
        .with_lock(|reservations_guard| {
            for (id, metadata) in reservations {
                reservations_guard.insert(id, metadata);
            }
            Ok(())
        })
        .await
        .map_err(|e: HypernodeError| hyper_err!(Store, "Failed to update reservations: {}", e))?;

    info!(
        "Synced {} reservations in {:?}",
        total_reservations,
        time.elapsed()
    );

    for (reservation_id, metadata) in downloaded_reservations
        .iter()
        .filter(|(id, _)| reservations_in_challenge.contains(id))
    {
        let unlock_timestamp = metadata.reservation.liquidityUnlockedTimestamp;
        trigger
            .add(evm_block_trigger::EvmBlockTriggerRequestInput::new(
                *reservation_id,
                unlock_timestamp,
            ))
            .await?;
    }

    Ok(latest_block)
}

pub async fn exchange_event_listener(
    contract: Arc<RiftExchangeWebsocket>,
    trigger: Arc<EvmBlockTrigger>,
    mut start_index_block_height: u64,
    mut start_block_header_height: u64,
    active_reservations: Arc<ThreadSafeStore>,
) -> Result<()> {
    const RETRY_DELAY: Duration = Duration::from_secs(5);

    loop {
        let result = try_exchange_event_listener(
            Arc::clone(&contract),
            Arc::clone(&trigger),
            start_index_block_height,
            start_block_header_height,
            Arc::clone(&active_reservations),
        )
        .await;

        match result {
            Ok((new_index_height, new_header_height)) => {
                // Update the start heights for the next iteration
                start_index_block_height = new_index_height;
                start_block_header_height = new_header_height;
                info!("Event listener completed successfully. Restarting from block heights: index={}, header={}", start_index_block_height, start_block_header_height);
            }
            Err(e) => {
                error!(
                    "Error in event listener: {}. Retrying in {:?}...",
                    e, RETRY_DELAY
                );
                sleep(RETRY_DELAY).await;
            }
        }
    }
}

async fn try_exchange_event_listener(
    contract: Arc<RiftExchangeWebsocket>,
    trigger: Arc<EvmBlockTrigger>,
    start_index_block_height: u64,
    start_block_header_height: u64,
    active_reservations: Arc<ThreadSafeStore>,
) -> Result<(u64, u64)> {
    info!(
        "Rift deployed at: {} on chain ID: {}",
        contract.address(),
        &contract
            .provider()
            .get_chain_id()
            .await
            .map_err(|e| hyper_err!(Evm, "Failed to get chain ID: {}", e))?
    );

    // Create filters for each event.
    let swap_complete_filter = contract
        .SwapComplete_filter()
        .from_block(start_index_block_height)
        .watch()
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to create SwapComplete filter: {}", e))?;
    let liquidity_reserved_filter = contract
        .LiquidityReserved_filter()
        .from_block(start_index_block_height)
        .watch()
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to create LiquidityReserved filter: {}", e))?;
    let proof_proposed_filter = contract
        .ProofSubmitted_filter()
        .from_block(start_index_block_height)
        .watch()
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to create ProofSubmitted filter: {}", e))?;
    let blocks_added_filter = contract
        .BlocksAdded_filter()
        .from_block(start_block_header_height)
        .watch()
        .await
        .map_err(|e| hyper_err!(Evm, "Failed to create BlocksAdded filter: {}", e))?;

    // Convert the filters into streams.
    let mut swap_complete_stream = swap_complete_filter.into_stream();
    let mut liquidity_reserved_stream = liquidity_reserved_filter.into_stream();
    let mut proof_proposed_stream = proof_proposed_filter.into_stream();
    let mut blocks_added_stream = blocks_added_filter.into_stream();

    // Use a HashSet to keep track of already-processed logs
    let processed_logs = Arc::new(Mutex::new(HashSet::new()));

    let mut current_index_block_height = start_index_block_height;
    let mut current_block_header_height = start_block_header_height;

    loop {
        tokio::select! {
            Some(log) = swap_complete_stream.next() => {
                let log_data = log.clone().map_err(|e| hyper_err!(Evm, "Failed to clone SwapComplete log: {}", e))?;
                let log_identifier = (log_data.1.block_number, log_data.1.transaction_index, log_data.1.log_index);

                let mut processed_logs_guard = processed_logs.lock().await;
                if !processed_logs_guard.contains(&log_identifier) {
                    processed_logs_guard.insert(log_identifier);
                    drop(processed_logs_guard);

                    let swap_reservation_index = log_data.0.swapReservationIndex;
                    info!("SwapComplete with reservation index: {:?}", &swap_reservation_index);
                    active_reservations.with_lock(|reservations_guard| {
                        reservations_guard.remove(swap_reservation_index);
                    }).await;
                }
                current_index_block_height = log_data.1.block_number.unwrap();
            }
            Some(log) = liquidity_reserved_stream.next() => {
                let log_data = log.clone().map_err(|e| hyper_err!(Evm, "Failed to clone LiquidityReserved log: {}", e))?;
                let log_identifier = (log_data.1.block_number, log_data.1.transaction_index, log_data.1.log_index);

                let mut processed_logs_guard = processed_logs.lock().await;
                if !processed_logs_guard.contains(&log_identifier) {
                    processed_logs_guard.insert(log_identifier);
                    drop(processed_logs_guard);

                    info!("LiquidityReserved w/ reservation index: {:?}", &log_data.0.swapReservationIndex);
                    let swap_reservation_index = log_data.0.swapReservationIndex;
                    let reservation = download_reservation(swap_reservation_index, Arc::clone(&contract)).await?;
                    active_reservations.with_lock(|reservations_guard| {
                        reservations_guard.insert(swap_reservation_index, reservation.1.clone());
                    }).await;
                }
                current_index_block_height = log_data.1.block_number.unwrap();
            }
            Some(log) = proof_proposed_stream.next() => {
                let log_data = log.clone().map_err(|e| hyper_err!(Evm, "Failed to clone ProofSubmitted log: {}", e))?;
                let log_identifier = (log_data.1.block_number, log_data.1.transaction_index, log_data.1.log_index);

                let mut processed_logs_guard = processed_logs.lock().await;
                if !processed_logs_guard.contains(&log_identifier) {
                    processed_logs_guard.insert(log_identifier);
                    drop(processed_logs_guard);

                    info!("ProofSubmitted w/ reservation index: {:?}", &log_data.0.swapReservationIndex);
                    let swap_reservation_index = log_data.0.swapReservationIndex;
                    let reservation_metadata = download_reservation(swap_reservation_index, Arc::clone(&contract)).await?;
                    active_reservations.with_lock(|reservations_guard| {
                        reservations_guard.insert(swap_reservation_index, reservation_metadata.1.clone());
                    }).await;
                    trigger.add(evm_block_trigger::EvmBlockTriggerRequestInput::new(
                        swap_reservation_index,
                        reservation_metadata.1.reservation.liquidityUnlockedTimestamp
                    )).await?;
                }
                current_index_block_height = log_data.1.block_number.unwrap();
            }
            Some(log) = blocks_added_stream.next() => {
                let log_data = log.clone().map_err(|e| hyper_err!(Evm, "Failed to clone BlocksAdded log: {}", e))?;
                let log_identifier = (log_data.1.block_number, log_data.1.transaction_index, log_data.1.log_index);

                let mut processed_logs_guard = processed_logs.lock().await;
                if !processed_logs_guard.contains(&log_identifier) {
                    processed_logs_guard.insert(log_identifier);
                    drop(processed_logs_guard);

                    let blocks_added = log_data.0;
                    let count = blocks_added.count;
                    let end_block_height = u64::from_be_bytes((blocks_added.startBlockHeight + count).to_be_bytes::<32>()[32-8..].try_into().unwrap());
                    info!("BlocksAdded w/ confirmation height: {:?} and safe height: {:?}", end_block_height, blocks_added.startBlockHeight);
                    download_safe_bitcoin_headers(
                        Arc::clone(&contract),
                        Arc::clone(&active_reservations),
                        Some(end_block_height),
                        Some(u64::from_be_bytes(blocks_added.count.to_be_bytes::<32>()[32-8..].try_into().unwrap()) as usize)).await?;
                }
                current_block_header_height = log_data.1.block_number.unwrap();
            }
            else => {
                info!("All event streams have closed. Returning current block heights.");
                return Ok((current_index_block_height, current_block_header_height));
            }
        }
    }
}
