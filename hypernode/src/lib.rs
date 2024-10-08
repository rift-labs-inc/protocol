pub mod btc_indexer;
pub mod btc_rpc;
pub mod constants;
pub mod core;
pub mod error;
pub mod evm_block_trigger;
pub mod evm_indexer;
pub mod node;
pub mod proof_broadcast;
pub mod proof_builder;

use clap::Parser;

pub type Result<T> = std::result::Result<T, error::HypernodeError>;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct HypernodeArgs {
    /// Ethereum RPC websocket URL for indexing and proposing proofs onchain
    #[arg(short, long, env)]
    pub evm_ws_rpc: String,

    /// Bitcoin RPC URL for indexing
    #[arg(short, long, env)]
    pub btc_rpc: String,

    /// Ethereum private key for signing hypernode initiated transactions
    #[arg(short, long, env)]
    pub private_key: String,

    /// Rift Exchange contract address
    #[arg(short, long, env)]
    pub rift_exchange_address: String,

    /// Ethereum RPC concurrency limit
    #[arg(short, long, env, default_value = "10")]
    pub evm_rpc_concurrency: usize,

    /// Bitcoin RPC concurrency limit
    #[arg(short, long, env, default_value = "10")]
    pub btc_rpc_concurrency: usize,

    /// Bitcoin new block polling interval in seconds
    #[arg(short, long, env, default_value = "30")]
    pub btc_polling_interval: u64,

    /// Average time between EVM blocks in seconds
    #[arg(short, long, env, default_value = "12")]
    pub evm_block_time: u64,

    /// Average time between Bitcoin blocks in seconds
    #[arg(short, long, env, default_value = "600")]
    pub btc_block_time: u64,

    /// Enable mock proof generation
    #[arg(short, long, env, default_value = "false")]
    pub mock_proof: bool,

    /// Proof generation concurrency limit
    #[arg(short, long, env, default_value = "1")]
    pub proof_gen_concurrency: usize,

    /// Utilize Flashbots to prevent frontrunning on propose + release transactions (recommended
    /// for public mempool EVM chains that support Flashbots)
    #[arg(short, long, env, default_value = "false")]
    pub flashbots: bool,

    /// Flashbots relay URL, required if flashbots is enabled, will only be utilized when
    /// broadcasting transactions
    #[arg(short, long, env)]
    pub flashbots_relay_rpc: Option<String>,
}
