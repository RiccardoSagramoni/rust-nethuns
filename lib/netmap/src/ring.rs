use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use crate::bindings::{netmap_ring, netmap_slot, nm_ring_next};
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
    netmap_ring: NonNull<netmap_ring>,
}

impl NetmapRing {
    /// Try to create a new `NetmapRing` object by a raw pointer.
    /// Return error if the pointer is null.
    pub fn new(ptr: NonNull<netmap_ring>) -> Self {
        Self { netmap_ring: ptr }
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
    
    
    /// Given the current value of one index of the ring slots
    /// (`head`, `cur`, `tail`), move it ahead of one position
    /// in a circular manner.
    ///
    /// # Arguments
    /// * `index` - the current value of one of the indexes (`head`, `cur`, `tail`)
    ///
    /// # Safety
    /// When calling this method, you have to ensure that all of the following is true:
    /// * `index` is the current value of `head`, `cur` or `tail`
    pub unsafe fn nm_ring_next(&self, index: u32) -> u32 {
        nm_ring_next(self.netmap_ring.as_ptr(), index)
    }
    
    /// Get a slot by its index.
    pub fn get_slot(&self, index: usize) -> Result<NetmapSlot, String> {
        // [SAFETY] Check for out-of-bounds
        if index >= self.num_slots as _ {
            return Err(format!(
                "[get_slot] index {index} out of bounds ({})",
                self.num_slots
            ));
        }
        
        let slot_array = std::ptr::addr_of!(self.slot) as *mut netmap_slot;
        Ok(NetmapSlot::new(
            NonNull::new(unsafe { slot_array.add(index) })
                .ok_or("[get_slot] slot pointer is null".to_owned())?,
        ))
    }
}

impl Deref for NetmapRing {
    type Target = netmap_ring;
    
    fn deref(&self) -> &Self::Target {
        // [SAFETY] Safety requirements met thanks to
        // the usage of `NonNull` to wrap the raw pointer.
        unsafe { self.netmap_ring.as_ref() }
    }
}

impl DerefMut for NetmapRing {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // [SAFETY] Safety requirements met thanks to
        // the usage of `NonNull` to wrap the raw pointer.
        unsafe { self.netmap_ring.as_mut() }
    }
}

/// # Safety
/// No one besides us has the raw pointer, so we can
/// safely transfer the ownership to another thread
unsafe impl Send for NetmapRing {}
