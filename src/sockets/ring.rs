use std::cell::RefCell;
use std::rc::Rc;

use super::ring_slot::NethunsRingSlot;


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
        for _i in 0..nslots {
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
            
            if !($ring.tail != $ring.head
                && slot.inuse.load(atomic::Ordering::Acquire) == 0)
            {
                break;
            }
            
            $free_macro!($s, slot);
            $ring.tail += 1;
        }
    };
}
pub(crate) use nethuns_ring_free_slots;
