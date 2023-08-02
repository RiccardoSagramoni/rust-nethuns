use std::ops::{Deref, DerefMut};

use crate::bindings::netmap_ring;

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
    pub r: *mut netmap_ring,
}

impl NetmapRing {
    pub fn try_new(ptr: *mut netmap_ring) -> Result<Self, ()> {
        if ptr.is_null() {
            return Err(());
        }
        Ok(Self {
            r: ptr,
        })
    }
}

impl Deref for NetmapRing {
    type Target = netmap_ring;
    
    fn deref(&self) -> &Self::Target {
        assert!(!self.r.is_null());
        unsafe { &*self.r }
    }
}

impl DerefMut for NetmapRing {
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(!self.r.is_null());
        unsafe { &mut *self.r }
    }
}
