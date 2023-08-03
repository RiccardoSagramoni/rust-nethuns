use std::error;

use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum NethunsOpenError {
    #[error("[try_new] invalid options: {0}")]
    InvalidOptions(String),
    #[error("[try_new] allocation error: {0}")]
    AllocationError(String),
}

#[derive(Clone, Debug, Error)]
pub enum NethunsBindError {
    #[error("[bind] error of the I/O framework: {0}")]
    FrameworkError(String),
    #[error("[bind] error caused by an illegal or inappropiate argument: {0}")]
    IllegalArgument(String),
    #[error("[bind] error caused by nethuns: {0}")]
    NethunsError(String),
}

#[derive(Clone, Debug, Error)]
pub enum NethunsRecvError {
    #[error("[recv] you must execute bind(...) before using the socket")]
    NonBinded,
    #[error("[recv] socket not in RX mode")]
    NotRx,
    #[error("[recv] socket in use by another thread")]
    InUse,
    #[error("[recv] no packets have been received")]
    NoPacketsAvailable,
    #[error("[recv] filtered")] // TODO improve
    Filtered,
    #[error("[bind] unexpected error: {0}")]
    NethunsError(String),
}
