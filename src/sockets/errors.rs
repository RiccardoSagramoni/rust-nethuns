use std::io;

use core::fmt::Debug;
use thiserror::Error;

/// Error type for [crate::sockets::nethuns_socket_open]
#[derive(Debug, Error)]
pub enum NethunsOpenError {
    #[error("[open] invalid options: {0}")]
    InvalidOptions(String),
    #[error("[open] an unexpected error occurred: {0}")]
    Error(String),
}

/// Error type for [crate::sockets::BindableNethunsSocket::bind]
#[derive(Debug, Error)]
pub enum NethunsBindError {
    #[error(
        "[bind] error caused by an illegal or inappropriate argument: {0}"
    )]
    IllegalArgument(String),
    #[error("[bind] error of the I/O framework: {0}")]
    FrameworkError(String),
    #[error("[bind] an unexpected error occurred: {0}")]
    Error(String),
}

/// Error type for [crate::sockets::NethunsSocket::recv]
#[derive(Debug, Error)]
pub enum NethunsRecvError {
    #[error("[recv] socket not in RX mode")]
    NotRx,
    #[error("[recv] ring in use")]
    InUse,
    #[error("[recv] no packets have been received")]
    NoPacketsAvailable,
    #[error("[recv] the received packet has been filtered out")]
    PacketFiltered,
    #[error("[recv] error of the I/O framework: {0}")]
    FrameworkError(String),
    #[error("[recv] an unexpected error occurred: {0}")]
    Error(String),
}

/// Error type for [crate::sockets::NethunsSocket::send]
#[derive(Debug, Error)]
pub enum NethunsSendError {
    #[error("[send] socket not in TX mode")]
    NotTx,
    #[error("[send] ring in use")]
    InUse,
    #[error("[send] an unexpected error occurred: {0}")]
    Error(String),
}


/// Error type for [crate::sockets::NethunsSocket::flush]
#[derive(Debug, Error)]
pub enum NethunsFlushError {
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


/// Error type for [crate::sockets::base::pcap::NethunsSocketPcapTrait::open]
#[derive(Debug, Error)]
pub enum NethunsPcapOpenError {
    // STANDARD_PCAP_READER
    #[error("[pcap_open] could not open pcap file for writing (enable `NETHUNS_USE_BUILTIN_PCAP_READER` feature to use builtin pcap reader)")]
    WriteModeNotSupported,
    #[error("[pcap_open] error while parsing pcap file: {0}")]
    PcapError(String),
    
    // BUILTIN_PCAP_READER
    #[error("[pcap_open] magic pcap_file_header not supported ({0:02x})")]
    MagicNotSupported(u32),
    #[error("[pcap_open] error while using file: {0}")]
    FileError(#[from] io::Error),
}

impl<I> From<pcap_parser::PcapError<I>> for NethunsPcapOpenError
where
    I: Debug + Sized,
{
    fn from(e: pcap_parser::PcapError<I>) -> Self {
        NethunsPcapOpenError::PcapError(format!("{:?}", e))
    }
}


/// Error type for [crate::sockets::base::pcap::NethunsSocketPcapTrait::read]
#[derive(Debug, Error)]
pub enum NethunsPcapReadError {
    #[error("[pcap_read] head ring in use")]
    InUse,
    
    // STANDARD_PCAP_READER
    #[error("[pcap_read] error while parsing pcap file: {0}")]
    PcapError(String),
    
    // BUILTIN_PCAP_READER
    #[error("[pcap_read] error during access to file: {0}")]
    FileError(io::Error),
    #[error("[pcap_read] end of file")]
    Eof,
}

impl<I> From<pcap_parser::PcapError<I>> for NethunsPcapReadError
where
    I: Debug + Sized,
{
    fn from(e: pcap_parser::PcapError<I>) -> Self {
        match e {
            pcap_parser::PcapError::Eof => NethunsPcapReadError::Eof,
            _ => NethunsPcapReadError::PcapError(format!("{:?}", e)),
        }
    }
}

impl From<io::Error> for NethunsPcapReadError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::UnexpectedEof => NethunsPcapReadError::Eof,
            _ => NethunsPcapReadError::FileError(e),
        }
    }
}


/// Error type for [crate::sockets::base::pcap::NethunsSocketPcapTrait::write]
#[derive(Debug, Error)]
pub enum NethunsPcapWriteError {
    // STANDARD_PCAP_READER
    #[error("[pcap_write] operation not supported")]
    NotSupported,
    
    // BUILTIN_PCAP_READER
    #[error("[pcap_write] error during access to file: {0}")]
    FileError(#[from] io::Error),
}


/// Error type for [crate::sockets::base::pcap::NethunsSocketPcapTrait::store]
#[derive(Debug, Error)]
pub enum NethunsPcapStoreError {
    // STANDARD_PCAP_READER
    #[error("[pcap_store] operation not supported")]
    NotSupported,
    
    // BUILTIN_PCAP_READER
    #[error("[pcap_store] error during access to file: {0}")]
    FileError(#[from] io::Error),
}


/// Error type for [crate::sockets::base::pcap::NethunsSocketPcapTrait::rewind]
#[derive(Debug, Error)]
pub enum NethunsPcapRewindError {
    // STANDARD_PCAP_READER
    #[error("[pcap_rewind] operation not supported")]
    NotSupported,
    
    // BUILTIN_PCAP_READER
    #[error("[pcap_rewind] error while using file: {0}")]
    FileError(#[from] io::Error),
}
