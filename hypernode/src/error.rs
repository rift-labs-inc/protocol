use crate::Result;
use eyre::eyre;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HypernodeError {
    #[error("Rpc error: {0}")]
    RpcError(String),
    #[error("Bitcoin RPC error: {0}")]
    BitcoinRpc(String),
    #[error("Evm error: {0}")]
    Evm(String),
    #[error("Decoding error: {0}")]
    Decode(String),
    #[error("Internal storage error: {0}")]
    Store(String),
    #[error("Conversion error: {0}")]
    Conversion(String),
    #[error("Queue error: {0}")]
    Queue(String),
    #[error("Proof broadcast error: {0}")]
    ProofBroadcast(String),
    #[error("Proof generation error: {0}")]
    ProofGeneration(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Indexer error: {0}")]
    Indexer(String),
    #[error("Listener error: {0}")]
    Listener(String),
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<eyre::Report> for HypernodeError {
    fn from(err: eyre::Report) -> Self {
        HypernodeError::Unknown(err.to_string())
    }
}

#[macro_export]
macro_rules! hyper_err {
    ($variant:ident, $msg:expr) => {
        HypernodeError::$variant($msg.to_string())
    };
    ($variant:ident, $fmt:expr, $($arg:tt)*) => {
        HypernodeError::$variant(format!($fmt, $($arg)*))
    };
}

pub fn to_eyre_result<T>(result: Result<T>) -> Result<T> {
    Ok(result.map_err(|e| eyre!(e))?)
}

pub trait ErrorExt<T> {
    fn wrap_err_app(self, msg: &str) -> Result<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ErrorExt<T> for std::result::Result<T, E> {
    fn wrap_err_app(self, msg: &str) -> Result<T> {
        self.map_err(|e| hyper_err!(Unknown, "{}: {}", msg, e))
    }
}
