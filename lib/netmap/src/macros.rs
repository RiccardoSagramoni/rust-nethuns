use std::ops::Deref;

use crate::bindings::{netmap_if, netmap_ring};
use crate::ring::NetmapRing;


/// This macro is equivalent to the C macro `__NETMAP_OFFSET`.
/// It executes **unsafe** code, so it must be wrapped in unsafe blocks.
macro_rules! __netmap_offset {
    ($type: ident, $ptr: expr, $off: expr) => {
        ($ptr as *const libc::c_char).add($off as _) as *const _ as *mut $type
    };
}


/// Equivalent to `NETMAP_TXRING`
/// 
/// # Safety
/// `nifp` must be a pointer to a valid `netmap_if` object
/// and `index` must be a valid index for a ring slot.
/// 
/// # Panics
/// If `nifp` is null or `index` is out-of-bounds
#[inline(always)]
pub unsafe fn netmap_txring(
    nifp: *mut netmap_if,
    index: usize,
) -> *mut netmap_ring {
    assert!(!nifp.is_null());
    assert!(index < unsafe { (*nifp).ni_tx_rings as _ });
    
    let offset = {
        let ptr = (*nifp).ring_ofs.as_ptr();
        assert!(!ptr.is_null());
        
        let ptr = ptr.add(index);
        ptr.read_unaligned()
    };
    
    __netmap_offset!(netmap_ring, nifp, offset)
}


/// Equivalent to `NETMAP_RXRING`
///
/// # Safety
/// `nifp` must be a pointer to a valid `netmap_if` object
/// and `index` must be a valid index for a ring slot.
/// 
/// # Panics
/// If `nifp` is null or `index` is out-of-bounds
#[inline(always)]
pub unsafe fn netmap_rxring(
    nifp: *mut netmap_if,
    index: usize,
) -> *mut netmap_ring {
    assert!(!nifp.is_null());
    assert!(index < unsafe { (*nifp).ni_rx_rings as _ });
    
    let offset = unsafe {
        let ptr = (*nifp).ring_ofs.as_ptr();
        assert!(!ptr.is_null());
        
        let ptr = ptr
            .add(index)
            .add((*nifp).ni_tx_rings as _)
            .add((*nifp).ni_host_tx_rings as _);
        ptr.read_unaligned()
    };
    unsafe { __netmap_offset!(netmap_ring, nifp, offset) }
}


/// Equivalent to `NETMAP_BUF`
///
/// # Safety
/// `index` must be a valid index for a buffer in the netmap ring
#[inline(always)]
pub unsafe fn netmap_buf(
    ring: &NetmapRing,
    index: usize,
) -> *const libc::c_char {
    (ring.deref() as *const _ as *const libc::c_char)
        .add(ring.buf_ofs as _)
        .add(index * ring.nr_buf_size as usize)
}

/// Returns a buffer which contains a packet as a slice of `u8`.
///
/// This macro is **unsafe**.
///
/// # Safety
/// Must be used only to read a packet from the netmap ring,
/// since it converts the raw pointer got from `netmap_buf`
/// into a slice assuming that the pointer is valid and the
/// size of the slice is equals to the packets size.
#[macro_export]
macro_rules! netmap_buf_pkt {
    ($ring: expr, $index: expr) => {
        std::slice::from_raw_parts(
            netmap_buf(&$ring, $index as _) as *const u8,
            $ring.nr_buf_size as _,
        )
    };
}


#[cfg(test)]
mod test {
    use std::mem::size_of;
    
    #[test]
    fn test_netmap_offset() {
        let x = 0;
        let x_ptr = &x as *const i32 as *mut i32;
        
        assert_eq!(unsafe { __netmap_offset!(i32, x_ptr, 0) }, x_ptr);
        
        assert_eq!(
            unsafe { __netmap_offset!(i32, x_ptr, size_of::<i32>()) },
            unsafe { x_ptr.add(1) }
        );
        
        assert_eq!(
            unsafe { __netmap_offset!(i32, x_ptr, 10 * size_of::<i32>()) },
            unsafe { x_ptr.add(10) }
        );
    }
}
