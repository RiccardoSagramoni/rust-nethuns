use std::cell::RefCell;
use std::cmp;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

use super::Pkthdr;

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
    #[inline(always)]
    pub fn get_slot(&self, index: usize) -> Rc<RefCell<NethunsRingSlot>> {
        self.rings
            .iter()
            .cycle()
            .nth(index)
            .expect(
                "Index out of bounds in a cyclic iterator should be impossible",
            )
            .clone()
    }
    
    
    /// Get the index of a slot in the ring, given its reference.
    ///
    /// # Returns
    /// * `Some(index)` - On success.
    /// * `None` - If the slot is not in the ring.
    #[inline(always)]
    pub fn get_idx_slot(
        &self,
        rc_slot: &Rc<RefCell<NethunsRingSlot>>,
    ) -> Option<usize> {
        self.rings
            .iter()
            .position(|slot: _| Rc::ptr_eq(slot, rc_slot))
    }
    
    
    /// Get the number of slots in the ring.
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.rings.len()
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
            .cycle()
            .skip(pos)
            .take(cmp::min(self.rings.len() - 1, 32))
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
        let slot = self.get_slot(self.head as _);
        self.head += 1;
        slot
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
    pub fn nethuns_send_slot(&self, id: u64, len: usize) -> bool {
        let rc_slot = self.get_slot(id as _);
        let mut slot = rc_slot.borrow_mut();
        if slot.inuse.load(Ordering::Acquire) != 0 {
            return false;
        }
        slot.len = len as _;
        slot.inuse.store(1, Ordering::Release);
        true
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
            let rc_slot = $ring.get_slot($ring.tail as _);
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


/// Get size of the RX ring.
#[inline(always)]
pub fn rxring_get_size(socket: &dyn NethunsSocket) -> Option<usize> {
    let rx_ring = match &socket.socket_base().rx_ring {
        Some(r) => r,
        None => return None,
    };
    Some(rx_ring.size())
}


/// Get size of the TX ring.
#[inline(always)]
pub fn txring_get_size(socket: &dyn NethunsSocket) -> Option<usize> {
    let tx_ring = match &socket.socket_base().tx_ring {
        Some(r) => r,
        None => return None,
    };
    Some(tx_ring.size())
}
