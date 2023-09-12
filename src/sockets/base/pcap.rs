mod constants;

use cfg_if::cfg_if;

use crate::sockets::errors::{NethunsPcapOpenError, NethunsPcapReadError};
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
    fn write(&mut self) -> Result<(), String>;
    
    /// TODO doc
    fn store(&mut self) -> Result<(), String>;
    
    /// TODO doc
    fn rewind(&mut self) -> Result<(), String>;
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
