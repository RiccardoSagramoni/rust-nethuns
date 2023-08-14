use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::AtomicU8;

use crate::NethunsSocket;


/// Ring abstraction for Nethuns sockets.
#[derive(Debug)]
pub struct NethunsRing {
    pub pktsize: usize,
    
    pub head: u64,
    pub tail: u64,
    
    rings: Vec<Rc<RefCell<NethunsRingSlot>>>,
}


impl NethunsRing {
    /// Create a new `NethunsRing` object.
    ///
    /// Equivalent to `nethuns_make_ring` from the original C library.
    #[inline(always)]
    pub fn new(nslots: usize, pktsize: usize) -> NethunsRing {
        // Allocate the slots for the ring
        let mut rings = Vec::with_capacity(nslots);
        for _ in 0..nslots {
            rings.push(Rc::new(RefCell::new(
                NethunsRingSlot::default_with_packet_size(pktsize),
            )));
        }
        
        NethunsRing {
            pktsize,
            head: 0,
            tail: 0,
            rings,
        }
    }
    
    
    /// Get a reference to a slot in the ring, given its index.
    ///
    /// Equivalent to `nethuns_get_slot` from the original C library.
    #[inline(always)]
    pub fn get_slot(
        self: &NethunsRing,
        index: usize,
    ) -> Rc<RefCell<NethunsRingSlot>> {
        let index = index % self.rings.len();
        self.rings[index].clone()
    }
    
    
    /// Get the number of slots in the ring.
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.rings.len()
    }
}


/// Ring slot of a Nethuns socket.
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


/// Free all the currently unused slots in the ring.
///
/// # Arguments
/// * `s` - A reference to the `NethunsSocket` object.
/// * `ring` - A reference to the `NethunsRing` object.
/// * `free_macro` - The name of the macro to call to free the slots.
macro_rules! nethuns_ring_free_slots {
    ($s: expr, $ring: expr, $free_macro: ident) => {
        loop {
            let rc_slot = $ring.get_slot($ring.tail as usize);
            let slot = rc_slot.borrow();
            
            if $ring.tail == $ring.head
                || slot.inuse.load(atomic::Ordering::Acquire) != 0
            {
                break;
            }
            
            $free_macro!($s, slot);
            $ring.tail += 1;
        }
    };
}
pub(crate) use nethuns_ring_free_slots;

use super::Pkthdr;


/// Get size of the TX ring.
#[inline(always)]
pub fn txring_get_size(socket: &dyn NethunsSocket) -> Option<usize> {
    let tx_ring = match &socket.socket_base().tx_ring {
        Some(r) => r,
        None => return None,
    };
    Some(tx_ring.size())
}
