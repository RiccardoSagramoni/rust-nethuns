use std::cell::RefCell;
use std::cmp;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

use super::{NethunsSocket, Pkthdr};

use crate::misc::circular_buffer::CircularCloneBuffer;


/// Ring abstraction for Nethuns sockets.
#[derive(Debug)]
pub struct NethunsRing {
    pktsize: usize,
    
    pub(super) rings: CircularCloneBuffer<Rc<RefCell<NethunsRingSlot>>>,
}


impl NethunsRing {
    /// Create a new `NethunsRing` object.
    ///
    /// Equivalent to `nethuns_make_ring` from the original C library.
    #[inline(always)]
    pub fn new(nslots: usize, pktsize: usize) -> NethunsRing {
        let builder = || {
            Rc::new(RefCell::new(NethunsRingSlot::default_with_packet_size(
                pktsize,
            )))
        };
        
        NethunsRing {
            pktsize,
            rings: CircularCloneBuffer::new(nslots, &builder),
        }
    }
    
    
    /// Get a reference to a slot in the ring, given its index.
    #[inline(always)]
    pub fn get_slot(&self, index: usize) -> Rc<RefCell<NethunsRingSlot>> {
        self.rings.get(index)
    }
    
    
    /// Get the index of a slot in the ring, given its reference.
    #[inline(always)]
    pub fn get_idx_slot(
        &self,
        rc_slot: &Rc<RefCell<NethunsRingSlot>>,
    ) -> Option<usize> {
        // FIXME: this is inefficient. How can we improve it?
        self.rings
            .iter()
            .take(self.rings.size())
            .position(|slot: _| Rc::ptr_eq(slot, rc_slot))
    }
    
    
    /// Get the number of slots in the ring.
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.rings.size()
    }
    
    /// Get the packet size
    #[inline(always)]
    pub fn pktsize(&self) -> usize {
        self.pktsize
    }
    
    
    /// Get the number of the consecutive available slots
    /// in the ring, starting from the given position.
    ///
    /// The returned value is capped to 32.
    #[inline(always)]
    pub fn num_free_slots(&self, pos: usize) -> usize {
        let mut total = 0_usize;
        
        for slot in self
            .rings
            .iter()
            .skip(pos)
            .take(cmp::min(self.size() - 1, 32))
        {
            if slot.borrow().inuse.load(Ordering::Acquire) == 0 {
                total += 1;
            } else {
                break;
            }
        }
        
        total
    }
    
    
    /// Get a reference to the head slot in the ring
    /// and shift the head to the following slot.
    pub fn next_slot(&mut self) -> Rc<RefCell<NethunsRingSlot>> {
        self.rings.pop_unchecked()
    }
    
    
    /// Mark the packet contained in a specific slot of a TX ring
    /// as *ready for transmission*, by setting to 1 the `inuse` field.
    ///
    /// # Arguments
    /// * `id` - The id of the slot which contains the packet to send.
    /// * `len` - The length of the packet.
    ///
    /// # Returns
    /// * `true` - On success.
    /// * `false` - If the slot is already in use.
    #[inline(always)]
    pub fn nethuns_send_slot(&self, id: usize, len: usize) -> bool {
        let rc_slot = self.get_slot(id as _);
        let mut slot = rc_slot.borrow_mut();
        if slot.inuse.load(Ordering::Acquire) != 0 {
            return false;
        }
        slot.len = len;
        slot.inuse.store(1, Ordering::Release);
        true
    }
}


/// Ring slot of a Nethuns socket.
#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub pkthdr: Pkthdr,
    pub id: usize, // FIXME: purpose??
    /// In-use flag => `0`: not in use; `1`: in use (a thread is reading a packet); `2`: in-flight (a thread is sending a packet)
    pub inuse: AtomicU8,
    pub len: usize,
    
    pub packet: Vec<u8>,
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
            let rc_slot = $ring.get_slot($ring.rings.tail());
            let slot = rc_slot.borrow();
            
            if $ring.rings.is_empty()
                || slot.inuse.load(atomic::Ordering::Acquire) != 0
            {
                break;
            }
            
            $free_macro!($s, slot);
            $ring.rings.advance_tail();
        }
    };
}
pub(super) use nethuns_ring_free_slots;


/// Get size of the RX ring.
#[inline(always)]
pub fn rxring_get_size(socket: &dyn NethunsSocket) -> Option<usize> {
    let rx_ring = match &socket.base().rx_ring {
        Some(r) => r,
        None => return None,
    };
    Some(rx_ring.size())
}


/// Get size of the TX ring.
#[inline(always)]
pub fn txring_get_size(socket: &dyn NethunsSocket) -> Option<usize> {
    let tx_ring = match &socket.base().tx_ring {
        Some(r) => r,
        None => return None,
    };
    Some(tx_ring.size())
}
