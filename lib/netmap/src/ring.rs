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
#[derive(Debug, Default)]
pub struct NetmapRing {
    pub r: Box<netmap_ring>,
}

impl NetmapRing {
    pub unsafe fn from_raw(ptr: *mut netmap_ring) -> Self {
        assert!(!ptr.is_null());
        Self {
            r: unsafe { Box::from_raw(ptr) },
        }
    }
}
