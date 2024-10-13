use crate::core::{EvmHttpProvider, RiftExchangeWebsocket};
use crate::core::{RiftExchange, ThreadSafeStore};
use crate::error::HypernodeError;
use crate::evm_indexer;
use crate::{hyper_err, Result};
use alloy::primitives::{FixedBytes, Uint, U256};
use alloy::providers::WalletProvider;
use alloy::sol_types::SolValue;
use bitcoin::Block;
use rift_core::btc_light_client::AsLittleEndianBytes;
use std::fmt::Debug;
use std::ops::Index;

use bitcoin::hashes::Hash;
use bitcoin::hex::DisplayHex;
use crypto_bigint::{Encoding, U256 as SP1OptimizedU256};
use json_patch::diff;
use log::{debug, error, info};
use rift_lib::{self, AsRiftOptimizedBlock};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum ProofBroadcastInput {
    Reservation {
        reservation_id: U256,
    },
    BlockProof {
        safe_chainwork: U256,
        safe_block_height: u64,
        blocks: Vec<Block>,
        retarget_block: Block,
        retarget_block_height: u64,
        solidity_proof: Vec<u8>,
        public_inputs: Vec<u8>,
    },
}

impl ProofBroadcastInput {
    pub fn new_reservation(reservation_id: U256) -> Self {
        ProofBroadcastInput::Reservation { reservation_id }
    }

    pub fn new_block_proof(
        safe_chainwork: U256,
        safe_block_height: u64,
        blocks: Vec<Block>,
        retarget_block: Block,
        retarget_block_height: u64,
        solidity_proof: Vec<u8>,
        public_inputs: Vec<u8>,
    ) -> Self {
        ProofBroadcastInput::BlockProof {
            safe_chainwork,
            safe_block_height,
            blocks,
            retarget_block,
            retarget_block_height,
            solidity_proof,
            public_inputs,
        }
    }
}

pub struct ProofBroadcastQueue {
    sender: mpsc::UnboundedSender<ProofBroadcastInput>,
}

impl ProofBroadcastQueue {
    pub fn new(
        store: Arc<ThreadSafeStore>,
        flashbots_provider: Arc<Option<EvmHttpProvider>>,
        contract: Arc<RiftExchangeWebsocket>,
        debug_url: &str,
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queue = ProofBroadcastQueue { sender };
        tokio::spawn(ProofBroadcastQueue::consume_task(
            receiver,
            store,
            flashbots_provider,
            contract,
            debug_url.to_string(),
        ));
        queue
    }

    pub fn add(&self, proof_args: ProofBroadcastInput) -> Result<()> {
        self.sender
            .send(proof_args)
            .map_err(|e| hyper_err!(Queue, "Failed to add to proof broadcast queue: {}", e))
    }

    async fn consume_task(
        mut receiver: mpsc::UnboundedReceiver<ProofBroadcastInput>,
        store: Arc<ThreadSafeStore>,
        flashbots_provider: Arc<Option<EvmHttpProvider>>,
        contract: Arc<RiftExchangeWebsocket>,
        debug_url: String,
    ) {
        let provider = contract.provider();

        info!(
            "Hypernode address: {}",
            provider.wallet().default_signer().address()
        );

        while let Some(item) = receiver.recv().await {
            if let Err(e) =
                Self::process_item(item, &store, &flashbots_provider, &contract, &debug_url).await
            {
                error!("Failed to process proof broadcast item: {}", e);
            }
        }
    }

    async fn process_item(
        item: ProofBroadcastInput,
        store: &Arc<ThreadSafeStore>,
        flashbots_provider: &Arc<Option<EvmHttpProvider>>,
        contract: &Arc<RiftExchangeWebsocket>,
        debug_url: &str,
    ) -> Result<()> {
        match item {
            ProofBroadcastInput::Reservation { reservation_id } => {
                info!("Processing proof broadcast item: {}", reservation_id);
                Self::process_reservation(
                    reservation_id,
                    store,
                    flashbots_provider,
                    contract,
                    debug_url,
                )
                .await
            }
            ProofBroadcastInput::BlockProof {
                safe_chainwork,
                safe_block_height,
                blocks,
                retarget_block,
                retarget_block_height,
                solidity_proof,
                public_inputs,
            } => {
                info!("Processing block proof broadcast");
                Self::process_block_proof(
                    safe_chainwork,
                    safe_block_height,
                    blocks,
                    retarget_block,
                    retarget_block_height,
                    solidity_proof,
                    public_inputs,
                    flashbots_provider,
                    contract,
                    debug_url,
                )
                .await
            }
        }
    }

    async fn process_reservation(
        reservation_id: U256,
        store: &Arc<ThreadSafeStore>,
        flashbots_provider: &Arc<Option<EvmHttpProvider>>,
        contract: &Arc<RiftExchangeWebsocket>,
        debug_url: &str,
    ) -> Result<()> {
        info!("Processing proof broadcast item: {}", reservation_id);
        let reservation_metadata = store
            .with_lock(|store| store.get(reservation_id).cloned())
            .await
            .ok_or_else(|| hyper_err!(Store, "Reservation not found: {}", reservation_id))?;

        let solidity_proof = reservation_metadata.proof.ok_or_else(|| {
            hyper_err!(
                ProofBroadcast,
                "Proof not found for reservation: {}",
                reservation_id
            )
        })?;
        let btc_initial = reservation_metadata.btc_initial.ok_or_else(|| {
            hyper_err!(
                ProofBroadcast,
                "BTC initial not found for reservation: {}",
                reservation_id
            )
        })?;
        let btc_final = reservation_metadata.btc_final.ok_or_else(|| {
            hyper_err!(
                ProofBroadcast,
                "BTC final not found for reservation: {}",
                reservation_id
            )
        })?;

        let mut bitcoin_tx_id = btc_initial.txid;
        bitcoin_tx_id.reverse();

        let proposed_block_height = btc_initial.proposed_block_height;
        let safe_block_height = btc_final.safe_block_height;
        let confirmation_block_height = btc_final.confirmation_height;
        let public_inputs_encoded = reservation_metadata.public_inputs.ok_or_else(|| {
            hyper_err!(
                ProofBroadcast,
                "Public inputs not found for reservation: {}",
                reservation_id
            )
        })?;

        let (block_hashes, chainworks) = Self::prepare_block_data(
            &btc_final.blocks,
            safe_block_height,
            &btc_final.safe_block_chainwork,
        )?;

        Self::validate_public_inputs(
            reservation_id,
            bitcoin_tx_id.into(),
            FixedBytes(
                btc_final
                    .blocks
                    .index(((proposed_block_height as u64) - (safe_block_height as u64)) as usize)
                    .header
                    .merkle_root
                    .to_byte_array()
                    .to_little_endian(),
            ),
            safe_block_height as u32,
            proposed_block_height,
            confirmation_block_height,
            &block_hashes,
            &chainworks,
            Arc::clone(contract),
            &public_inputs_encoded,
            true,
        )
        .await?;

        let txn_calldata = contract
            .submitSwapProof(
                reservation_id,
                bitcoin_tx_id.into(),
                FixedBytes(
                    btc_final
                        .blocks
                        .index(
                            ((proposed_block_height as u64) - (safe_block_height as u64)) as usize,
                        )
                        .header
                        .merkle_root
                        .to_byte_array()
                        .to_little_endian(),
                ),
                safe_block_height as u32,
                proposed_block_height,
                confirmation_block_height,
                block_hashes,
                chainworks,
                solidity_proof.into(),
            )
            .calldata()
            .to_owned();

        Self::broadcast_transaction(
            contract,
            flashbots_provider,
            &txn_calldata,
            debug_url,
            "submitSwapProof",
        )
        .await
    }

    async fn process_block_proof(
        safe_chainwork: U256,
        safe_block_height: u64,
        blocks: Vec<Block>,
        _retarget_block: Block,
        _retarget_block_height: u64,
        solidity_proof: Vec<u8>,
        public_inputs: Vec<u8>,
        flashbots_provider: &Arc<Option<EvmHttpProvider>>,
        contract: &Arc<RiftExchangeWebsocket>,
        debug_url: &str,
    ) -> Result<()> {
        let (block_hashes, chainworks) = Self::prepare_block_data(
            &blocks,
            safe_block_height,
            &safe_chainwork.to_be_bytes::<32>(),
        )?;

        let confirmation_block_height = safe_block_height + blocks.len() as u64 - 1;
        let proposed_block_height = 0; //sentinel value

        // info all the inputs
        info!("safe_block_height: {}", safe_block_height);
        info!("confirmation_block_height: {}", confirmation_block_height);
        info!("block count: {}", blocks.len());

        // Validate public inputs
        Self::validate_public_inputs(
            Uint::<256, 4>::ZERO, // Placeholder for swap_reservation_index (not used for block proofs)
            FixedBytes::default(), // Placeholder for bitcoin_tx_id (not used for block proofs)
            FixedBytes::default(), // Placeholder for merkle_root (not used for block proofs)
            safe_block_height as u32,
            proposed_block_height,
            confirmation_block_height,
            &block_hashes,
            &chainworks,
            Arc::clone(contract),
            &public_inputs,
            false,
        )
        .await?;

        let txn_calldata = contract
            .proveBlocks(
                safe_block_height as u32,
                confirmation_block_height,
                block_hashes,
                chainworks,
                solidity_proof.into(),
            )
            .calldata()
            .to_owned();

        Self::broadcast_transaction(
            contract,
            flashbots_provider,
            &txn_calldata,
            debug_url,
            "proveBlocks",
        )
        .await
    }

    fn prepare_block_data(
        blocks: &[Block],
        safe_block_height: u64,
        safe_block_chainwork: &[u8],
    ) -> Result<(Vec<FixedBytes<32>>, Vec<Uint<256, 4>>)> {
        let block_hashes = blocks
            .iter()
            .map(|block| {
                let mut block_hash = block.block_hash().to_raw_hash().to_byte_array();
                block_hash.reverse();
                FixedBytes::from_slice(&block_hash)
            })
            .collect::<Vec<_>>();

        let chainworks = rift_lib::transaction::get_chainworks(
            blocks
                .iter()
                .zip(safe_block_height..safe_block_height + block_hashes.len() as u64)
                .map(|(block, height)| block.as_rift_optimized_block(height))
                .collect::<Vec<_>>()
                .as_slice(),
            SP1OptimizedU256::from_be_slice(safe_block_chainwork),
        )
        .iter()
        .map(|chainwork| Uint::<256, 4>::from_be_bytes(chainwork.to_be_bytes()))
        .collect::<Vec<_>>();

        Ok((block_hashes, chainworks))
    }

    async fn broadcast_transaction(
        contract: &Arc<RiftExchangeWebsocket>,
        flashbots_provider: &Arc<Option<EvmHttpProvider>>,
        txn_calldata: &[u8],
        debug_url: &str,
        function_name: &str,
    ) -> Result<()> {
        debug!("{} calldata: {}", function_name, txn_calldata.as_hex());

        let tx_hash = if let Some(flashbots_provider) = flashbots_provider.as_ref() {
            evm_indexer::broadcast_transaction_via_flashbots(
                contract,
                flashbots_provider,
                txn_calldata,
            )
            .await?
        } else {
            evm_indexer::broadcast_transaction(contract, txn_calldata, debug_url).await?
        };

        info!(
            "{} broadcasted with evm tx hash: {}",
            function_name,
            tx_hash.to_string()
        );
        Ok(())
    }

    // validate that circuit generated public inputs match what the contract will generate
    async fn validate_public_inputs(
        swap_reservation_index: Uint<256, 4>,
        bitcoin_tx_id: FixedBytes<32>,
        merkle_root: FixedBytes<32>,
        safe_block_height: u32,
        proposed_block_height: u64,
        confirmation_block_height: u64,
        block_hashes: &[FixedBytes<32>],
        block_chainworks: &[Uint<256, 4>],
        contract: Arc<RiftExchangeWebsocket>,
        circuit_generated_public_inputs_encoded: &[u8],
        is_transaction_proof: bool,
    ) -> Result<()> {
        // call the buildPublicInputs function in the contract
        let contract_generated_public_inputs_decoded = if is_transaction_proof {
            contract
                .buildPublicInputs(
                    swap_reservation_index,
                    bitcoin_tx_id,
                    merkle_root,
                    safe_block_height,
                    proposed_block_height,
                    confirmation_block_height,
                    block_hashes.to_vec(),
                    block_chainworks.to_vec(),
                    is_transaction_proof,
                )
                .call()
                .await
                .map_err(|e| hyper_err!(Evm, "Failed to call buildPublicInputs: {}", e))?
                ._0
        } else {
            contract
                .buildBlockProofPublicInputs(
                    safe_block_height,
                    confirmation_block_height,
                    block_hashes.to_vec(),
                    block_chainworks.to_vec(),
                )
                .call()
                .await
                .map_err(|e| hyper_err!(Evm, "Failed to call buildBlockProofPublicInputs: {}", e))?
                ._0
        };

        let contract_generated_public_inputs_encoded =
            <RiftExchange::ProofPublicInputs as SolValue>::abi_encode(
                &contract_generated_public_inputs_decoded,
            );

        let circuit_generated_public_inputs_decoded =
            <RiftExchange::ProofPublicInputs as SolValue>::abi_decode(
                circuit_generated_public_inputs_encoded,
                false,
            )
            .map_err(|e| {
                hyper_err!(
                    ProofBroadcast,
                    "Failed to decode circuit generated public inputs: {}",
                    e
                )
            })?;

        if contract_generated_public_inputs_encoded != circuit_generated_public_inputs_encoded {
            let contract_json =
                serde_json::to_value(contract_generated_public_inputs_decoded).unwrap();
            let circuit_json =
                serde_json::to_value(circuit_generated_public_inputs_decoded).unwrap();
            let patch = diff(&circuit_json, &contract_json);
            return Err(hyper_err!(
                ProofBroadcast,
                "Public inputs generated by the contract do not match the expected public inputs. Diff: {}", patch
            ));
        }

        Ok(())
    }
}
