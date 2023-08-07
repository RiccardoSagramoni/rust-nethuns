use std::sync::PoisonError;

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
    #[error("[bind] lock acquisition error: {0}")]
    LockError(String),
    #[error("[bind] error caused by nethuns: {0}")]
    NethunsError(String),
}

#[derive(Debug, Error)]
pub enum NethunsRecvError {
    #[error("[recv] you must execute bind(...) before using the socket")]
    NonBinded,
    #[error("[recv] socket not in RX mode")]
    NotRx,
    #[error("[recv] socket in use by another thread")]
    InUse,
    #[error("[recv] no packets have been received")]
    NoPacketsAvailable,
    #[error("[recv] lock acquisition error: {0}")]
    LockError(String),
    #[error("[recv] filtered")] // TODO improve
    PacketFiltered,
    #[error("[recv] unexpected error: {0}")]
    NethunsError(String),
}

impl<T> From<PoisonError<T>> for NethunsRecvError {
    fn from(e: PoisonError<T>) -> Self {
        Self::LockError(e.to_string())
    }
}
