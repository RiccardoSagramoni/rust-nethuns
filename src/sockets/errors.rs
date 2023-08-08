use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum NethunsOpenError {
    #[error("[open] invalid options: {0}")]
    InvalidOptions(String),
    #[error("[open] an unexpected error occurred: {0}")]
    Error(String),
}

#[derive(Clone, Debug, Error)]
pub enum NethunsBindError {
    #[error("[bind] error caused by an illegal or inappropriate argument: {0}")]
    IllegalArgument(String),
    #[error("[bind] error of the I/O framework: {0}")]
    FrameworkError(String),
    #[error("[bind] an unexpected error occurred: {0}")]
    Error(String),
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
    #[error("[recv] filtered")] // TODO improve
    PacketFiltered,
    #[error("[recv] error of the I/O framework: {0}")]
    FrameworkError(String),
    #[error("[recv] an unexpected error occurred: {0}")]
    Error(String),
}
