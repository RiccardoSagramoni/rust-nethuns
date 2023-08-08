use std::sync::atomic::AtomicBool;

use super::Pkthdr;


/// Ring slot of a Nethuns socket.
#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub pkthdr: Pkthdr,
    pub id: u64,
    pub inuse: AtomicBool,
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
