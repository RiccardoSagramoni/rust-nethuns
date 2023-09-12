use std::io;

use core::fmt::Debug;
use thiserror::Error;

/// Error type for [super::nethuns_socket_open]
#[derive(Debug, Error)]
pub enum NethunsOpenError {
    #[error("[open] invalid options: {0}")]
    InvalidOptions(String),
    #[error("[open] an unexpected error occurred: {0}")]
    Error(String),
}

/// Error type for [super::BindableNethunsSocket::bind]
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

/// Error type for [super::NethunsSocket::recv]
#[derive(Debug, Error)]
pub enum NethunsRecvError {
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

/// Error type for [super::NethunsSocket::send]
#[derive(Debug, Error)]
pub enum NethunsSendError {
    #[error("[send] socket not in TX mode")]
    NotTx,
    #[error("[send] ring in use")]
    InUse,
    #[error("[send] an unexpected error occurred: {0}")]
    Error(String),
}


/// Error type for [super::NethunsSocket::flush]
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


/// Error type for [super::base::pcap::NethunsSocketPcap::open]
#[derive(Debug, Error)]
pub enum NethunsPcapOpenError {
    #[error("[pcap_open] could not open pcap file for writing (use built-in pcap option)")]
    WriteModeNotSupported,
    #[error("[pcap_open] unable to open file: {0}")]
    FileError(#[from] io::Error),
    #[error("[pcap_open] error while parsing pcap file: {0}")]
    PcapError(String),
}

impl<I> From<pcap_parser::PcapError<I>> for NethunsPcapOpenError
where
    I: Debug + Sized,
{
    fn from(e: pcap_parser::PcapError<I>) -> Self {
        NethunsPcapOpenError::PcapError(format!("{:?}", e))
    }
}


/// Error type for [super::base::pcap::NethunsSocketPcap::read]
#[derive(Debug, Error)]
pub enum NethunsPcapReadError {
    #[error("[pcap_read] head ring in use")]
    InUse,
    #[error("[pcap_read] error while parsing pcap file: {0}")]
    PcapError(String),
}

impl<I> From<pcap_parser::PcapError<I>> for NethunsPcapReadError
where
    I: Debug + Sized,
{
    fn from(e: pcap_parser::PcapError<I>) -> Self {
        NethunsPcapReadError::PcapError(format!("{:?}", e))
    }
}


/// Error type for [super::base::pcap::NethunsSocketPcap::write]
#[derive(Debug, Error)]
pub enum NethunsPcapWriteError {
    #[error("[pcap_write] operation not supported")]
    NotSupported(),
}


/// Error type for [super::base::pcap::NethunsSocketPcap::store]
#[derive(Debug, Error)]
pub enum NethunsPcapStoreError {
    #[error("[pcap_store] operation not supported")]
    NotSupported(),
}


/// Error type for [super::base::pcap::NethunsSocketPcap::rewind]
#[derive(Debug, Error)]
pub enum NethunsPcapRewindError {
    #[error("[pcap_rewind] operation not supported")]
    NotSupported(),
}
