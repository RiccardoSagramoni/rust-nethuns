//! Nethuns socket for packet capture (PCAP).

mod constants;

use core::fmt::Debug;
use std::cell::UnsafeCell;
use std::marker::PhantomData;

use cfg_if::cfg_if;
use derivative::Derivative;
use getset::CopyGetters;

use crate::misc::hybrid_rc::state::Shared;
use crate::misc::hybrid_rc::state_trait::RcState;
use crate::sockets::errors::{
    NethunsPcapOpenError, NethunsPcapReadError, NethunsPcapRewindError,
    NethunsPcapStoreError, NethunsPcapWriteError,
};
use crate::sockets::PkthdrTrait;
use crate::types::NethunsSocketOptions;

use super::base::{PcapRecvPacket, RecvPacketData};
use super::{Local, NethunsSocketBase, RecvPacket};


/// Nethuns socket for packet capture (PCAP).
///
/// Depending on the `NETHUNS_USE_BUILTIN_PCAP_READER` feature,
/// the implementation of this struct will use the standard pcap reader
/// (STANDARD_PCAP_READER) or a custom built-in pcap reader (BUILTIN_PCAP_READER).
#[derive(Debug)]
#[repr(transparent)]
pub struct NethunsSocketPcap<State: RcState> {
    inner: UnsafeCell<NethunsSocketPcapInner<State>>,
}

static_assertions::assert_impl_all!(NethunsSocketPcap<Shared>: Send);

impl<State: RcState> NethunsSocketPcap<State> {
    /// Open the socket for reading captured packets from a file.
    ///
    /// # Arguments
    /// * `opt`: socket options
    /// * `filename`: name of the pcap file
    /// * `writing_mode`: whether to open the file for writing
    ///
    /// # Returns
    /// * `Ok(NethunsSocketPcap)` - a new nethuns socket for pcap, in no error occurs.
    /// * `Err(NethunsPcapOpenError::WriteModeNotSupported)` - if writing mode is not supported (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::PcapError)` - if an error occurs while parsing the pcap file (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::FileError)` - if an error occurs while accessing the file (BUILTIN_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::MagicNotSupported)` - if the format of the pcap file is not supported (BUILTIN_PCAP_READER only).
    pub fn open(
        opt: NethunsSocketOptions,
        filename: &str,
        writing_mode: bool,
    ) -> Result<Self, NethunsPcapOpenError> {
        NethunsSocketPcapInner::open(opt, filename, writing_mode).map(|inner| {
            Self {
                inner: UnsafeCell::new(inner),
            }
        })
    }
    
    
    /// Write a packet already in pcap format to a pcap file.
    ///
    /// # Arguments
    /// * `header`: pcap header of the packet
    /// * `packet`: packet to write
    ///
    /// # Returns
    /// * `Ok(usize)` - the number of bytes written to the pcap file.
    /// * `Err(NethunsPcapWriteError::NotSupported)` - if the `NETHUNS_USE_BUILTIN_PCAP_READER` feature is not enabled (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapWriteError::FileError)` - if an I/O error occurs while accessing the file (BUILTIN_PCAP_READER only).
    pub fn write(
        &self,
        header: &nethuns_pcap_pkthdr,
        packet: &[u8],
    ) -> Result<usize, NethunsPcapWriteError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).write(header, packet) }
    }
    
    
    /// Store a packet received from a [`NethunsSocket`](crate::sockets::NethunsSocket) into a pcap file.
    ///
    /// # Arguments
    /// * `pkthdr`: packet header
    /// * `packet`: packet to store
    ///
    /// # Returns
    /// * `Ok(u32)` - the number of bytes written to the pcap file.
    /// * `Err(NethunsPcapWriteError::NotSupported)` - if the `NETHUNS_USE_BUILTIN_PCAP_READER` feature is not enabled (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapWriteError::FileError)` - if an I/O error occurs while accessing the file (BUILTIN_PCAP_READER only).
    pub fn store(
        &self,
        pkthdr: &dyn PkthdrTrait,
        packet: &[u8],
    ) -> Result<u32, NethunsPcapStoreError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).store(pkthdr, packet) }
    }
    
    
    /// Rewind the reader to the beginning of the pcap file.
    ///
    /// # Returns
    /// * `Ok(u64)` - the new position from the start of the file.
    /// * `Err(NethunsPcapRewindError::NotSupported)` - if the `NETHUNS_USE_BUILTIN_PCAP_READER` feature is not enabled (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapRewindError::FileError)` - if an I/O error occurs while accessing the file (BUILTIN_PCAP_READER only).
    pub fn rewind(&self) -> Result<u64, NethunsPcapRewindError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).rewind() }
    }
    
    
    /// Get an immutable reference to the nethuns base socket.
    pub fn base(&self) -> &NethunsSocketBase<State> {
        unsafe { &(*UnsafeCell::raw_get(&self.inner)).base }
    }
}

impl NethunsSocketPcap<Local> {
    /// Read a packet from the socket.
    ///
    /// # Returns
    /// * `Ok(RecvPacket<NethunsSocketPcap>)` - the packet read from the socket.
    /// * `Err(NethunsPcapReadError::InUse)` - if the ring buffer of the nethuns base socket is full.
    /// * `Err(NethunsPcapOpenError::PcapError)` - if an error occurs while parsing the pcap file (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::FileError)` - if an error occurs while accessing the file (BUILTIN_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::Eof)` - if the end of the file is reached.
    pub fn read(
        &self,
    ) -> Result<PcapRecvPacket<Local, Local>, NethunsPcapReadError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).read() }
            .map(|p| RecvPacket::new(p, PhantomData))
    }
}

impl NethunsSocketPcap<Shared> {
    /// Read a packet from the socket.
    ///
    /// # Returns
    /// * `Ok(RecvPacket<NethunsSocketPcap>)` - the packet read from the socket.
    /// * `Err(NethunsPcapReadError::InUse)` - if the ring buffer of the nethuns base socket is full.
    /// * `Err(NethunsPcapOpenError::PcapError)` - if an error occurs while parsing the pcap file (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::FileError)` - if an error occurs while accessing the file (BUILTIN_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::Eof)` - if the end of the file is reached.
    pub fn read(
        &self,
    ) -> Result<PcapRecvPacket<Shared, Shared>, NethunsPcapReadError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).read() }
            .map(|p| RecvPacket::new(p, PhantomData))
    }
}


/// Inner struct of the nethuns socket for packet capture (PCAP).
/// It implements the [`NethunsSocketPcapTrait`] trait.
///
/// The implementation is handled by the modules
#[allow(rustdoc::broken_intra_doc_links)]
#[doc = "[`reader_builtin`] or [`reader_pcap`],"]
/// depending on the value of the `NETHUNS_USE_BUILTIN_PCAP_READER` feature.
#[derive(Derivative)]
#[derivative(Debug)]
struct NethunsSocketPcapInner<State: RcState> {
    base: NethunsSocketBase<State>,
    
    #[derivative(Debug = "ignore")]
    reader: PcapReaderType,
    
    snaplen: u32,
    magic: u32,
}

static_assertions::assert_impl_all!(
    NethunsSocketPcapInner<Local>:
        NethunsSocketPcapTrait<Local>,
        LocalReadSocketPcapTrait
);
static_assertions::assert_not_impl_any!(
    NethunsSocketPcapInner<Local>:
        Send, Sync
);
static_assertions::assert_impl_all!(
    NethunsSocketPcapInner<Shared>:
        Send,
        NethunsSocketPcapTrait<Shared>,
        SharedReadSocketPcapTrait
);
static_assertions::assert_not_impl_all!(
    NethunsSocketPcapInner<Shared>:
        Sync
);

/// Public interface for [`NethunsSocketPcapInner`].
trait NethunsSocketPcapTrait<State: RcState>: Debug {
    /// Open the socket for reading captured packets from a file.
    ///
    /// # Arguments
    /// * `opt`: socket options
    /// * `filename`: name of the pcap file
    /// * `writing_mode`: whether to open the file for writing
    ///
    /// # Returns
    /// * `Ok(NethunsSocketPcap)` - a new nethuns socket for pcap, in no error occurs.
    /// * `Err(NethunsPcapOpenError::WriteModeNotSupported)` - if writing mode is not supported (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::PcapError)` - if an error occurs while parsing the pcap file (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::FileError)` - if an error occurs while accessing the file (BUILTIN_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::MagicNotSupported)` - if the format of the pcap file is not supported (BUILTIN_PCAP_READER only).
    fn open(
        opt: NethunsSocketOptions,
        filename: &str,
        writing_mode: bool,
    ) -> Result<Self, NethunsPcapOpenError>
    where
        Self: Sized;
    
    
    /// Write a packet already in pcap format to a pcap file.
    ///
    /// # Arguments
    /// * `header`: pcap header of the packet
    /// * `packet`: packet to write
    ///
    /// # Returns
    /// * `Ok(usize)` - the number of bytes written to the pcap file.
    /// * `Err(NethunsPcapWriteError::NotSupported)` - if the `NETHUNS_USE_BUILTIN_PCAP_READER` feature is not enabled (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapWriteError::FileError)` - if an I/O error occurs while accessing the file (BUILTIN_PCAP_READER only).
    fn write(
        &mut self,
        header: &nethuns_pcap_pkthdr,
        packet: &[u8],
    ) -> Result<usize, NethunsPcapWriteError>;
    
    
    /// Store a packet received from a [`NethunsSocket`](crate::sockets::NethunsSocket) into a pcap file.
    ///
    /// # Arguments
    /// * `pkthdr`: packet header
    /// * `packet`: packet to store
    ///
    /// # Returns
    /// * `Ok(u32)` - the number of bytes written to the pcap file.
    /// * `Err(NethunsPcapWriteError::NotSupported)` - if the `NETHUNS_USE_BUILTIN_PCAP_READER` feature is not enabled (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapWriteError::FileError)` - if an I/O error occurs while accessing the file (BUILTIN_PCAP_READER only).
    fn store(
        &mut self,
        pkthdr: &dyn PkthdrTrait,
        packet: &[u8],
    ) -> Result<u32, NethunsPcapStoreError>;
    
    
    /// Rewind the reader to the beginning of the pcap file.
    ///
    /// # Returns
    /// * `Ok(u64)` - the new position from the start of the file.
    /// * `Err(NethunsPcapRewindError::NotSupported)` - if the `NETHUNS_USE_BUILTIN_PCAP_READER` feature is not enabled (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapRewindError::FileError)` - if an I/O error occurs while accessing the file (BUILTIN_PCAP_READER only).
    fn rewind(&mut self) -> Result<u64, NethunsPcapRewindError>;
}

/// TODO
trait LocalReadSocketPcapTrait: NethunsSocketPcapTrait<Local> {
    /// Read a packet from the socket.
    ///
    /// # Returns
    /// * `Ok(RecvPacket<NethunsSocketPcap>)` - the packet read from the socket.
    /// * `Err(NethunsPcapReadError::InUse)` - if the ring buffer of the nethuns base socket is full.
    /// * `Err(NethunsPcapOpenError::PcapError)` - if an error occurs while parsing the pcap file (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::FileError)` - if an error occurs while accessing the file (BUILTIN_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::Eof)` - if the end of the file is reached.
    fn read(&mut self) -> Result<RecvPacketData<Local>, NethunsPcapReadError>;
}

/// TODO
trait SharedReadSocketPcapTrait: NethunsSocketPcapTrait<Shared> {
    /// Read a packet from the socket.
    ///
    /// # Returns
    /// * `Ok(RecvPacket<NethunsSocketPcap>)` - the packet read from the socket.
    /// * `Err(NethunsPcapReadError::InUse)` - if the ring buffer of the nethuns base socket is full.
    /// * `Err(NethunsPcapOpenError::PcapError)` - if an error occurs while parsing the pcap file (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::FileError)` - if an error occurs while accessing the file (BUILTIN_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::Eof)` - if the end of the file is reached.
    fn read(&mut self) -> Result<RecvPacketData<Shared>, NethunsPcapReadError>;
}


// Include the implementation of `NethunsSocketPcapTrait`
// according to the `NETHUNS_USE_BUILTIN_PCAP_READER` feature
cfg_if!(
    if #[cfg(feature="NETHUNS_USE_BUILTIN_PCAP_READER")] {
        mod reader_builtin;
        use reader_builtin::*;
    } else {
        mod reader_pcap;
        use reader_pcap::*;
    }
);


/// Pcap packet header
#[allow(non_camel_case_types, clippy::len_without_is_empty)]
#[derive(Clone, Copy, Debug, Default, CopyGetters)]
#[getset(get_copy = "pub")]
#[repr(C)] // needed for safe transmutation to &[u8] and for compatibility with C programs
pub struct nethuns_pcap_pkthdr {
    /// timestamp
    ts: nethuns_pcap_timeval,
    /// length of portion present
    caplen: u32,
    /// length of this packet (off wire)
    len: u32,
}


/// Patched pcap packet header for the Kuznetzov's implementation of TCPDUMP format
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, CopyGetters)]
#[getset(get_copy = "pub")]
#[repr(C)] // needed for safe transmutation to &[u8] and for compatibility with C programs
pub struct nethuns_pcap_patched_pkthdr {
    hdr: nethuns_pcap_pkthdr,
    index: i32,
    protocol: libc::c_ushort,
    pkt_type: libc::c_uchar,
}


#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, CopyGetters)]
#[getset(get_copy = "pub")]
#[repr(C)] // needed for safe transmutation to &[u8] and for compatibility with C programs
pub struct nethuns_pcap_timeval {
    tv_sec: i64,
    tv_usec: i64,
}
