// TODO check safety of raw pointers

use crate::bindings::{netmap_if, netmap_ring};

/// Equivalent to __NETMAP_OFFSET
#[inline(always)]
pub fn netmap_offset<T>(ptr: *const netmap_if, offset: usize) -> *mut T {
    let mut ptr = ptr as *const char;
    unsafe { ptr = ptr.add(offset) }
    ptr as *const libc::c_void as *mut T
}


/// Equivalent to NETMAP_TXRING
#[inline(always)]
pub unsafe fn netmap_txring(
    nifp: *mut netmap_if,
    index: usize,
) -> *mut netmap_ring {
    assert!(!nifp.is_null());
    
    let offset = unsafe {
        let ring_ofs_ptr = (*nifp).ring_ofs.as_ptr();
        assert!(!ring_ofs_ptr.is_null());
        let ring_ofs_ptr = ring_ofs_ptr.add(index);
        *ring_ofs_ptr as usize
    };
    netmap_offset::<netmap_ring>(nifp, offset)
}


/// Equivalent to NETMAP_RXRING
#[inline(always)]
pub unsafe fn netmap_rxring(
    nifp: *mut netmap_if,
    index: usize,
) -> *mut netmap_ring {
    assert!(!nifp.is_null());
    
    let offset = unsafe {
        let ring_ofs_ptr = (*nifp).ring_ofs.as_ptr();
        assert!(!ring_ofs_ptr.is_null());
        let index = index
            + (*nifp).ni_tx_rings as usize
            + (*nifp).ni_host_tx_rings as usize;
        let ring_ofs_ptr = ring_ofs_ptr.add(index);
        *ring_ofs_ptr as usize
    };
    netmap_offset::<netmap_ring>(nifp, offset)
}


/// Equivalent to C macro NETMAP_BUF(ring, index)
#[inline(always)]
pub fn netmap_buf(ring: &netmap_ring, index: usize) -> *const u8 {
    let byte_index =
        (index * ring.nr_buf_size as usize) + ring.buf_ofs as usize;
    
    unsafe {
        (ring as *const netmap_ring as *const u8).add(byte_index)
    }
}
