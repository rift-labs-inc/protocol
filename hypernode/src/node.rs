use crate::constants::RESERVATION_DURATION_HOURS;
use crate::core::{
    EvmHttpProvider, EvmWebsocketProvider, RiftExchange, RiftExchangeWebsocket, ThreadSafeStore,
};
use crate::error::HypernodeError;
use crate::{btc_indexer, btc_rpc, evm_indexer, proof_broadcast, proof_builder};
use crate::{evm_block_trigger, HypernodeArgs};
use crate::{hyper_err, Result};
use alloy::rpc::client::ClientBuilder;
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

    let (contract, flashbots_provider) = create_providers_and_contract(
        &args.evm_ws_rpc,
        &args.private_key,
        rift_exchange_address,
        args.flashbots,
        args.flashbots_relay_rpc.as_deref(),
    )
    .await?;

    let btc_rpc = Arc::new(btc_rpc::BitcoinRpcClient::new(&args.btc_rpc));

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

    let trigger = Arc::new(evm_block_trigger::EvmBlockTrigger::new(
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
        Arc::clone(&trigger),
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
            Arc::clone(&trigger),
            synced_reservation_evm_height,
            synced_block_header_evm_height,
            Arc::clone(&safe_store)
        ),
        btc_indexer::block_listener(
            Arc::clone(&btc_rpc),
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

async fn create_providers_and_contract(
    evm_ws_rpc: &str,
    private_key_hex: &str,
    rift_exchange_address: alloy::primitives::Address,
    flashbots_enabled: bool,
    flashbots_relay_rpc: Option<&str>,
) -> Result<(Arc<RiftExchangeWebsocket>, Arc<Option<EvmHttpProvider>>)> {
    let private_key: [u8; 32] = hex::decode(private_key_hex.trim_start_matches("0x"))
        .map_err(|e| hyper_err!(Parse, "Failed to decode private key: {}", e))?
        .get(..32)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| hyper_err!(Parse, "Invalid private key length"))?;

    let ws = WsConnect::new(evm_ws_rpc);
    let ws = crate::core::RetryWsConnect(ws);
    let client = ClientBuilder::default()
        .pubsub(ws)
        .await
        .map_err(|e| hyper_err!(Connection, "Failed to connect to WebSocket: {}", e))?;

    let provider: Arc<EvmWebsocketProvider> = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(EthereumWallet::from(
                PrivateKeySigner::from_bytes(&private_key.into()).unwrap(),
            ))
            .on_client(client),
    );

    let contract: Arc<RiftExchangeWebsocket> =
        Arc::new(RiftExchange::new(rift_exchange_address, provider.clone()));

    let flashbots_provider: Arc<Option<EvmHttpProvider>> = Arc::new(if flashbots_enabled {
        let url = flashbots_relay_rpc.ok_or_else(|| {
            hyper_err!(
                Config,
                "Flashbots relay URL is required when flashbots is enabled"
            )
        })?;
        Some(
            ProviderBuilder::new()
                .with_recommended_fillers()
                .wallet(EthereumWallet::from(
                    PrivateKeySigner::from_bytes(&private_key.into()).unwrap(),
                ))
                .on_http(
                    url.parse()
                        .map_err(|e| hyper_err!(Parse, "Failed to parse Flashbots URL: {}", e))?,
                ),
        )
    } else {
        None
    });

    Ok((contract, flashbots_provider))
}
