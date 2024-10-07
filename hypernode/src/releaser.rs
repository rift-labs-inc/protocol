// Calls releaseLiquidity once enough evm blocks have passed
use crate::core::{EvmHttpProvider, RiftExchangeWebsocket};
use crate::evm_indexer::{broadcast_transaction, broadcast_transaction_via_flashbots};
use crate::Result;
use alloy::primitives::U256;
use alloy::providers::Provider;
use alloy::rpc::types::{Block, Transaction};
use futures::lock::Mutex;
use futures::StreamExt;
use log::{debug, info};
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug)]
pub struct ReleaserRequestInput {
    reservation_id: U256,
    unlock_timestamp: u64,
}

impl ReleaserRequestInput {
    pub fn new(reservation_id: U256, unlock_timestamp: u64) -> Self {
        ReleaserRequestInput {
            reservation_id,
            unlock_timestamp,
        }
    }
}

pub struct ReleaserQueue {
    release_queue: Arc<Mutex<Vec<ReleaserRequestInput>>>,
    flashbots_provider: Arc<Option<EvmHttpProvider>>,
    contract: Arc<RiftExchangeWebsocket>,
    debug_url: String,
}

impl ReleaserQueue {
    pub fn new(
        flashbots_provider: Arc<Option<EvmHttpProvider>>,
        contract: Arc<RiftExchangeWebsocket>,
        debug_url: &str,
    ) -> Arc<Self> {
        let queue = Arc::new(Self {
            release_queue: Arc::new(Mutex::new(Vec::new())),
            flashbots_provider,
            contract,
            debug_url: debug_url.to_string(),
        });

        ReleaserQueue::trigger_release_on_blocks(Arc::clone(&queue)).unwrap();

        queue
    }

    pub async fn add(&self, req: ReleaserRequestInput) -> Result<()> {
        let mut release_queue_handle = self.release_queue.lock().await;
        if !release_queue_handle
            .iter()
            .any(|r| r.reservation_id == req.reservation_id)
        {
            info!(
                "Added new release request for reservation ID: {}",
                &req.reservation_id
            );
            release_queue_handle.push(req);
        } else {
            info!(
                "Release request for reservation ID: {} already exists in the queue",
                req.reservation_id
            );
        }
        Ok(())
    }

    fn trigger_release_on_blocks(queue: Arc<Self>) -> Result<()> {
        tokio::spawn(async move {
            let provider = queue.contract.provider();
            let sub = provider.subscribe_blocks().await.unwrap();

            let mut stream = sub.into_stream();
            while let Some(block) = stream.next().await {
                // Here you can add the logic to process the queue based on the new block
                match queue.process_queue(block).await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Error processing queue: {:?}", e);
                    }
                }
            }
        });
        Ok(())
    }

    async fn release_liquidity(&self, reservation_id: U256) -> Result<()> {
        let txn_calldata = self
            .contract
            .releaseLiquidity(reservation_id)
            .calldata()
            .to_owned();

        let tx_hash = if let Some(flashbots_provider) = self.flashbots_provider.as_ref() {
            info!(
                "Broadcasting release for reservation index: {} via Flashbots",
                reservation_id
            );
            broadcast_transaction_via_flashbots(&self.contract, flashbots_provider, &txn_calldata)
                .await?
        } else {
            broadcast_transaction(&self.contract, &txn_calldata, &self.debug_url).await?
        };
        info!("Liquidity released with evm tx hash: {}", tx_hash);
        Ok(())
    }

    async fn process_queue(&self, block: Block<Transaction>) -> Result<()> {
        let current_timestamp = block.header.timestamp;
        let mut queue = self.release_queue.lock().await;

        // Separate ready and not ready items
        let (ready, not_ready): (Vec<_>, Vec<_>) = queue
            .drain(..)
            .partition(|req| current_timestamp > req.unlock_timestamp);

        if !ready.is_empty() {
            debug!("Processing queue at block timestamp: {}", current_timestamp);
            debug!(
                "Ready releaser items: {:?}",
                ready.iter().map(|r| r).collect::<Vec<_>>()
            );
        }
        if !not_ready.is_empty() {
            debug!("Processing queue at block timestamp: {}", current_timestamp);
            debug!(
                "Not ready releaser indexes: {:?}",
                not_ready.iter().map(|r| r).collect::<Vec<_>>()
            );
        }

        // Process all ready items concurrently
        let release_futures = ready.into_iter().map(|req| {
            let reservation_id = req.reservation_id;
            async move {
                match self.release_liquidity(reservation_id).await {
                    Ok(_) => {
                        info!(
                            "Successfully released liquidity for reservation ID: {}",
                            reservation_id
                        );
                        Ok(())
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to release liquidity for reservation ID: {}, Error: {:?}",
                            reservation_id,
                            e
                        );
                        Err(e)
                    }
                }
            }
        });

        // Wait for all release operations to complete
        let results: Vec<Result<()>> = futures::future::join_all(release_futures).await;

        // Log any errors that occurred during processing
        for result in results {
            if let Err(e) = result {
                log::error!("Error during batch processing: {:?}", e);
            }
        }

        // Update the queue with remaining items
        *queue = not_ready;

        Ok(())
    }
}
