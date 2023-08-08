use std::ops::{Deref, DerefMut};

use crate::bindings::{netmap_ring, netmap_slot, nm_ring_empty, nm_ring_next};
use crate::slot::NetmapSlot;

/// Safe wrapper for `netmap_ring` structure from the C library.
///
/// Implements transmit and receive rings, with read/write pointers,
/// metadata and an array of slots describing the buffers.
///
/// ```C
/// struct netmap_ring { /* (one per ring) */
///     ...
///     const uint32_t num_slots;   /* slots in each ring            */
///     const uint32_t nr_buf_size; /* size of each buffer           */
///     ...
///     uint32_t        head;       /* (u) first buf owned by user   */
///     uint32_t        cur;        /* (u) wakeup position           */
///     const uint32_t  tail;       /* (k) first buf owned by kernel */
///     ...
///     uint32_t        flags;
///     struct timeval  ts;         /* (k) time of last rxsync()     */
///     ...
///     struct netmap_slot slot[0]; /* array of slots                */
///     }
/// ```
#[derive(Debug)]
pub struct NetmapRing {
    netmap_ring: *mut netmap_ring,
}

impl NetmapRing {
    /// Try to create a new `NetmapRing` object by a raw pointer.
    /// Return error if the pointer is null.
    pub fn try_new(ptr: *mut netmap_ring) -> Result<Self, String> {
        if ptr.is_null() {
            return Err("[NetmapRing::try_new()] ptr is null".to_owned());
        }
        Ok(Self { netmap_ring: ptr })
    }
    
    
    /// Check if space is available in the ring. 
    /// 
    /// We use `self.head`, which points to the next netmap slot 
    /// to be published to netmap. It is possible that the applications 
    /// moves `self.cur` ahead of `self.tail` (e.g., by setting `self.cur` <== `self.tail`), 
    /// if it wants more slots than the ones currently available, 
    /// and it wants to be notified when more arrive.
    #[inline(always)]
    pub fn nm_ring_empty(&self) -> bool {
        self.head == self.tail
    }
    
    
    /// Given the current value of the index of the ring slots
    /// (`head`, `cur`, `tail`), move it ahead of one position
    /// in a circular manner.
    pub fn nm_ring_next(&self, i: u32) -> u32 {
        assert!(!self.netmap_ring.is_null());
        unsafe { nm_ring_next(self.netmap_ring, i) }
    }
}

impl Deref for NetmapRing {
    type Target = netmap_ring;
    
    fn deref(&self) -> &Self::Target {
        assert!(!self.netmap_ring.is_null());
        unsafe { &*self.netmap_ring }
    }
}

impl DerefMut for NetmapRing {
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(!self.netmap_ring.is_null());
        unsafe { &mut *self.netmap_ring }
    }
}

impl NetmapRing {
    pub fn get_slot(&self, index: usize) -> Result<NetmapSlot, String> {
        let slot_array = std::ptr::addr_of!(self.slot) as *mut netmap_slot;
        NetmapSlot::try_new(unsafe { slot_array.add(index) })
    }
}
