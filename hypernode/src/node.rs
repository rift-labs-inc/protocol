use crate::constants::RESERVATION_DURATION_HOURS;
use crate::core::{
    EvmHttpProvider, EvmWebsocketProvider, RiftExchange, RiftExchangeWebsocket, ThreadSafeStore,
};
use crate::error::HypernodeError;
use crate::{btc_indexer, evm_indexer, proof_broadcast, proof_builder};
use crate::{hyper_err, Result};
use crate::{releaser, HypernodeArgs};
use alloy::{
    network::EthereumWallet,
    providers::{ProviderBuilder, WsConnect},
    signers::local::PrivateKeySigner,
};
use std::{str::FromStr, sync::Arc};

pub async fn run(args: HypernodeArgs) -> Result<()> {
    let rift_exchange_address =
        alloy::primitives::Address::from_str(&args.rift_exchange_address)
            .map_err(|e| hyper_err!(Parse, "Failed to parse Rift exchange address: {}", e))?;

    let safe_store = Arc::new(ThreadSafeStore::new());

    let flashbots_url = if args.flashbots {
        Some(
            args.flashbots_relay_rpc
                .as_ref()
                .ok_or_else(|| {
                    hyper_err!(
                        Config,
                        "Flashbots relay URL is required when flashbots is enabled"
                    )
                })?
                .clone(),
        )
    } else {
        None
    };

    let private_key: [u8; 32] = hex::decode(args.private_key.trim_start_matches("0x"))
        .map_err(|e| hyper_err!(Parse, "Failed to decode private key: {}", e))?
        .get(..32)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| hyper_err!(Parse, "Invalid private key length"))?;

    let provider: Arc<EvmWebsocketProvider> = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(EthereumWallet::from(
                PrivateKeySigner::from_bytes(&private_key.into()).unwrap(),
            ))
            .on_ws(WsConnect::new(&args.evm_ws_rpc))
            .await
            .map_err(|e| hyper_err!(Connection, "Failed to connect to WebSocket: {}", e))?,
    );

    let contract: Arc<RiftExchangeWebsocket> =
        Arc::new(RiftExchange::new(rift_exchange_address, provider.clone()));

    let flashbots_provider: Arc<Option<EvmHttpProvider>> = Arc::new(match flashbots_url {
        None => None,
        Some(url) => Some(
            ProviderBuilder::new()
                .with_recommended_fillers()
                .wallet(EthereumWallet::from(
                    PrivateKeySigner::from_bytes(&private_key.into()).unwrap(),
                ))
                .on_http(
                    url.parse()
                        .map_err(|e| hyper_err!(Parse, "Failed to parse Flashbots URL: {}", e))?,
                ),
        ),
    });

    let proof_broadcast_queue = Arc::new(proof_broadcast::ProofBroadcastQueue::new(
        Arc::clone(&safe_store),
        Arc::clone(&flashbots_provider),
        Arc::clone(&contract),
        args.evm_ws_rpc.as_ref(),
    ));

    let proof_gen_queue = Arc::new(proof_builder::ProofGenerationQueue::new(
        Arc::clone(&safe_store),
        Arc::clone(&proof_broadcast_queue),
        args.mock_proof,
        args.proof_gen_concurrency,
    ));

    let release_queue = Arc::new(releaser::ReleaserQueue::new(
        Arc::clone(&flashbots_provider),
        Arc::clone(&contract),
        args.evm_ws_rpc.as_ref(),
    ));

    let (start_evm_block_height, start_btc_block_height) = tokio::try_join!(
        evm_indexer::find_block_height_from_time(
            &contract,
            RESERVATION_DURATION_HOURS,
            args.evm_block_time
        ),
        btc_indexer::find_block_height_from_time(
            &args.btc_rpc,
            RESERVATION_DURATION_HOURS,
            args.btc_block_time
        )
    )
    .map_err(|e| hyper_err!(Indexer, "Failed to find starting block heights: {}", e))?;

    let synced_reservation_evm_height = evm_indexer::sync_reservations(
        Arc::clone(&contract),
        Arc::clone(&safe_store),
        Arc::clone(&release_queue),
        start_evm_block_height,
        args.evm_rpc_concurrency,
    )
    .await
    .map_err(|e| hyper_err!(Indexer, "Failed to sync reservations: {}", e))?;

    let synced_block_header_evm_height = evm_indexer::download_safe_bitcoin_headers(
        Arc::clone(&contract),
        Arc::clone(&safe_store),
        None,
        None,
    )
    .await
    .map_err(|e| hyper_err!(Indexer, "Failed to download safe Bitcoin headers: {}", e))?;

    tokio::try_join!(
        evm_indexer::exchange_event_listener(
            Arc::clone(&contract),
            Arc::clone(&release_queue),
            synced_reservation_evm_height,
            synced_block_header_evm_height,
            Arc::clone(&safe_store)
        ),
        btc_indexer::block_listener(
            &args.btc_rpc,
            start_btc_block_height,
            args.btc_polling_interval,
            Arc::clone(&safe_store),
            Arc::clone(&proof_gen_queue),
            args.btc_rpc_concurrency
        )
    )
    .map_err(|e| hyper_err!(Listener, "Event listener or block listener failed: {}", e))?;

    Ok(())
}

