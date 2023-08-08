use std::sync::atomic::AtomicU8;

use super::Pkthdr;


/// Ring slot of a Nethuns socket.
#[repr(C)]
#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub pkthdr: Pkthdr,
    pub id: u64,
    /// In-use flag => `0`: not in use; `1`: in use (a thread is reading a packet); `2`: in-flight (a thread is sending a packet)
    pub inuse: AtomicU8,
    pub len: i32,
    
    pub packet: Vec<libc::c_uchar>,
}


impl NethunsRingSlot {
    /// Get a new `NethunsRingSlot` with `packet` initialized 
    /// with a given packet size.
    pub fn default_with_packet_size(pktsize: usize) -> Self {
        NethunsRingSlot {
            packet: vec![0; pktsize],
            ..Default::default()
        }
    }
}
