mod socket;


pub use socket::*;


use crate::sockets::errors::{NethunsPcapOpenError, NethunsPcapReadError};
use crate::types::NethunsSocketOptions;

use super::{NethunsSocketBase, RecvPacket};


/// TODO
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
