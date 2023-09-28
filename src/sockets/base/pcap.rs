mod constants;

use cfg_if::cfg_if;
use derivative::Derivative;

use crate::sockets::errors::{
    NethunsPcapOpenError, NethunsPcapReadError, NethunsPcapRewindError,
    NethunsPcapStoreError, NethunsPcapWriteError,
};
use crate::sockets::PkthdrTrait;
use crate::types::NethunsSocketOptions;

use super::{NethunsSocketBase, RecvPacket};


/// Nethuns socket for packet capture (PCAP).
pub struct NethunsSocketPcap {
    base: NethunsSocketBase,
    reader: PcapReaderType,
    snaplen: u32,
    magic: u32,
}

impl NethunsSocketPcap {
    pub fn base(&self) -> &NethunsSocketBase {
        &self.base
    }
    pub fn snaplen(&self) -> u32 {
        self.snaplen
    }
    pub fn magic(&self) -> u32 {
        self.magic
    }
}


/// Public interface for [NethunsSocketPcap].
/// 
/// Depending on the `NETHUNS_USE_BUILTIN_PCAP_READER` feature,
/// the implementation of this trait will use the standard pcap reader 
/// (STANDARD_PCAP_READER) or a custom built-in pcap reader (BUILTIN_PCAP_READER).
pub trait NethunsSocketPcapTrait {
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
    
    
    /// Read a packet from the socket.
    /// 
    /// # Returns
    /// * `Ok(RecvPacket)` - the packet read from the socket.
    /// * `Err(NethunsPcapReadError::InUse)` - if the ring buffer of the nethuns base socket is full.
    /// * `Err(NethunsPcapOpenError::PcapError)` - if an error occurs while parsing the pcap file (STANDARD_PCAP_READER only).
    /// * `Err(NethunsPcapOpenError::FileError)` - if an error occurs while accessing the file (BUILTIN_PCAP_READER only).
    fn read(&mut self) -> Result<RecvPacket, NethunsPcapReadError>;
    
    
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
    
    
    /// Store a packet received from a [crate::sockets::NethunsSocket] into a pcap file.
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
#[allow(non_camel_case_types)]
#[repr(C)] // needed for safe transmutation to &[u8] and for compatibility with C programs
#[derive(Debug, Derivative)]
#[derivative(Default)]
pub struct nethuns_pcap_pkthdr {
    #[derivative(Default(
        value = "pcap_sys::timeval { tv_sec: 0, tv_usec: 0 }"
    ))]
    /// timestamp
    pub ts: pcap_sys::timeval,
    /// length of portion present
    pub caplen: u32,
    /// length of this packet (off wire)
    pub len: u32,
}


/// Patched pcap packet header for the Kuznetzov's implementation of TCPDUMP format
#[allow(non_camel_case_types)]
#[repr(C)] // needed for safe transmutation to &[u8] and for compatibility with C programs
#[derive(Debug, Default)]
pub struct nethuns_pcap_patched_pkthdr {
    pub hdr: nethuns_pcap_pkthdr,
    pub index: i32,
    pub protocol: libc::c_ushort,
    pub pkt_type: libc::c_uchar,
}
