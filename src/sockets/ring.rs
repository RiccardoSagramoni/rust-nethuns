pub mod ring_slot;
pub use ring_slot::*;

use std::cmp;

use super::NethunsSocket;

use crate::misc::circular_buffer::CircularCloneBuffer;
use crate::misc::send_rc::SendRc;


/// Ring abstraction for Nethuns sockets.
#[derive(Debug)]
pub struct NethunsRing {
    pktsize: usize,
    
    pub(crate) rings: CircularCloneBuffer<SendRc<RingSlotMutex>>,
}


impl NethunsRing {
    /// Create a new `NethunsRing` object.
    ///
    /// Equivalent to `nethuns_make_ring` from the original C library.
    #[inline(always)]
    pub fn new(nslots: usize, pktsize: usize) -> NethunsRing {
        let builder = || SendRc::new(RingSlotMutex::new(pktsize));
        
        NethunsRing {
            pktsize,
            rings: CircularCloneBuffer::new(nslots, &builder),
        }
    }
    
    
    /// Get a reference to a slot in the ring, given its index.
    #[inline(always)]
    pub fn get_slot(&self, index: usize) -> SendRc<RingSlotMutex> {
        self.rings.get(index)
    }
    
    
    /// Get the index of a slot in the ring, given its reference.
    #[inline(always)]
    pub fn get_idx_slot(
        &self,
        rc_slot: &SendRc<RingSlotMutex>,
    ) -> Option<usize> {
        // FIXME: this is inefficient. Can we improve it?
        self.rings
            .iter()
            .take(self.rings.size())
            .position(|slot: _| SendRc::ptr_eq(slot, rc_slot))
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
    
    /// Check if the buffer is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.rings.is_empty()
    }
    
    /// Check if the buffer is full
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.rings.is_full()
    }
    
    /// Get the current head index
    #[inline(always)]
    pub fn head(&self) -> usize {
        self.rings.head()
    }
    
    /// Get the current tail index
    #[inline(always)]
    pub fn tail(&self) -> usize {
        self.rings.tail()
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
            if slot.status() == InUseStatus::Free {
                total += 1;
            } else {
                break;
            }
        }
        
        total
    }
    
    
    /// Get a reference to the head slot in the ring
    /// and shift the head to the following slot.
    pub fn next_slot(&mut self) -> SendRc<RingSlotMutex> {
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
        let slot = self.get_slot(id as _);
        if slot.status() != InUseStatus::Free {
            return false;
        }
        unsafe {
            slot.inner_mut().len = len;
        }
        slot.set_status(InUseStatus::Reading);
        true
    }
}


/// Free all the currently unused slots in the ring.
///
/// # Arguments
/// * `socket` - A reference to the `NethunsSocket` object.
/// * `ring` - A reference to the `NethunsRing` object.
/// * `free_macro` - The name of the macro to call to free the slots. It must exposed the following interface: `free_macro(socket, slot, block_id)`
macro_rules! nethuns_ring_free_slots {
    ($socket: expr, $ring: expr, $free_macro: ident) => {
        loop {
            let slot = $ring.get_slot($ring.rings.tail());
            
            if $ring.rings.is_empty()
                || slot.status() != crate::sockets::ring::InUseStatus::Free
            {
                break;
            }
            
            $free_macro!($socket, slot, slot.inner().id);
            $ring.rings.advance_tail();
        }
    };
}
pub(super) use nethuns_ring_free_slots;


/// Get size of the RX ring.
#[inline(always)]
pub fn rxring_get_size(socket: &dyn NethunsSocket) -> Option<usize> {
    let rx_ring = match &socket.base().rx_ring() {
        Some(r) => r,
        None => return None,
    };
    Some(rx_ring.size())
}


/// Get size of the TX ring.
#[inline(always)]
pub fn txring_get_size(socket: &dyn NethunsSocket) -> Option<usize> {
    let tx_ring = match &socket.base().tx_ring() {
        Some(r) => r,
        None => return None,
    };
    Some(tx_ring.size())
}
