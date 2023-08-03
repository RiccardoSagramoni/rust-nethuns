use super::Pkthdr;

#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub pkthdr: Pkthdr, // FIXME is it ok?
    pub id: u64,
    pub len: i32,
    
    pub packet: Vec<libc::c_uchar>,
}
// field inuse removed: use Mutex or RxLock instead


impl NethunsRingSlot {
    pub fn default_with_packet_size(pktsize: usize) -> Self {
        NethunsRingSlot {
            packet: vec![0; pktsize],
            ..Default::default()
         }
    }
}
