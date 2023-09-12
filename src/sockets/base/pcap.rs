mod socket;


pub use socket::*;


use super::NethunsSocketBase;


/// TODO
pub struct NethunsSocketPcap {
    base: NethunsSocketBase,
    reader: PcapReaderType,
    snaplen: u32,
    magic: u32,
}
