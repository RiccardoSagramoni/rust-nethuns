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
    #[error("[recv] ring in use")]
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

#[derive(Error, Debug)]
pub enum NethunsSendError {
    #[error("[send] you must execute bind(...) before using the socket")]
    NonBinded,
    #[error("[send] socket not in TX mode")]
    NotTx,
    #[error("[send] ring in use")]
    InUse,
    #[error("[send] an unexpected error occurred: {0}")]
    Error(String),
}


#[derive(Error, Debug)]
pub enum NethunsFlushError {
    #[error("[flush] you must execute bind(...) before using the socket")]
    NonBinded,
    #[error("[flush] socket not in TX mode")]
    NotTx,
    #[error("[flush] ring in use")]
    InUse,
    #[error("[flush] failed transmission: {0}")]
    FailedTransmission(String),
    #[error("[recv] error of the I/O framework: {0}")]
    FrameworkError(String),
    #[error("[flush] an unexpected error occurred: {0}")]
    Error(String),
}
