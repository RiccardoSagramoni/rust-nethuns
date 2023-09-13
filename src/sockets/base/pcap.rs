mod constants;

use cfg_if::cfg_if;
use derivative::Derivative;

use crate::sockets::PkthdrTrait;
use crate::sockets::errors::{NethunsPcapOpenError, NethunsPcapReadError, NethunsPcapWriteError, NethunsPcapRewindError, NethunsPcapStoreError};
use crate::types::NethunsSocketOptions;

use super::{NethunsSocketBase, RecvPacket};


/// TODO doc
pub struct NethunsSocketPcap {
    base: NethunsSocketBase,
    reader: PcapReaderType,
    snaplen: u32,
    magic: u32,
}

impl NethunsSocketPcap {
    pub fn snaplen(&self) -> u32 {
        self.snaplen
    }
    
    pub fn magic(&self) -> u32 {
        self.magic
    }
}


/// TODO doc
pub trait NethunsSocketPcapTrait {
    /// TODO doc
    fn open(
        opt: NethunsSocketOptions,
        filename: &str,
        writing_mode: bool,
    ) -> Result<Self, NethunsPcapOpenError>
    where
        Self: Sized;
    
    /// TODO doc
    fn read(&mut self) -> Result<RecvPacket, NethunsPcapReadError>;
    
    /// TODO doc
    fn write(
        &mut self,
        header: &nethuns_pcap_pkthdr,
        packet: &[u8],
    ) -> Result<usize, NethunsPcapWriteError>;
    
    /// TODO doc
    fn store(&mut self, pkthdr: &dyn PkthdrTrait, packet: &[u8]) -> Result<u32, NethunsPcapStoreError>;
    
    /// TODO doc
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


#[allow(non_camel_case_types)]
#[repr(C)]
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


#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Default)]
pub struct nethuns_pcap_patched_pkthdr {
    pub hdr: nethuns_pcap_pkthdr,
    pub index: i32,
    pub protocol: libc::c_ushort,
    pub pkt_type: libc::c_uchar,
}
