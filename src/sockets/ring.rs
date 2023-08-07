use std::cell::RefCell;
use std::rc::Rc;

use super::ring_slot::NethunsRingSlot;


#[derive(Debug)]
pub struct NethunsRing {
    pub size: usize,
    pub pktsize: usize,
    
    pub head: u64,
    pub tail: u64,
    
    rings: Vec<Rc<RefCell<NethunsRingSlot>>>,
}


impl NethunsRing {
    /// Equivalent to nethuns_make_ring
    #[inline(always)]
    pub fn try_new(
        nslots: usize,
        pktsize: usize,
    ) -> Result<NethunsRing, String> {
        let mut rings = Vec::with_capacity(nslots);
        for _i in 0..nslots {
            rings.push(Rc::new(RefCell::new(
                NethunsRingSlot::default_with_packet_size(pktsize),
            )));
        }
        
        Ok(NethunsRing {
            size: nslots,
            pktsize,
            head: 0,
            tail: 0,
            rings,
        })
    }
    
    
    /// Equivalent to nethuns_get_slot
    #[inline(always)]
    pub fn get_slot(
        self: &NethunsRing,
        n: usize,
    ) -> Rc<RefCell<NethunsRingSlot>> {
        let n = n % self.rings.len();
        self.rings[n].clone()
    }
}


/// TODO
macro_rules! nethuns_ring_free_slots {
    ($s: expr, $ring: expr, $slot: expr, $blocks_free_macro: ident) => {
        while $ring.tail != $ring.head
            && !$slot.inuse.load(atomic::Ordering::Acquire)
        {
            $blocks_free_macro!($s, $slot);
            $ring.tail += 1;
        }
    };
}
pub(crate) use nethuns_ring_free_slots;
