use thiserror::Error;

#[derive(Error, Debug)]
#[error("Bitcoin RPC failed to download data: {0}")]
pub struct BitcoinRpcError(pub String);
