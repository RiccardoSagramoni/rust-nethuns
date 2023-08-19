// TODO documentazione (soprattuto per la Safety)

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
pub(crate) use __netmap_offset;


/// Equivalent to `NETMAP_TXRING`
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
        *ring_ofs_ptr
    };
    unsafe { __netmap_offset!(netmap_ring, nifp, offset) }
}


/// Equivalent to `NETMAP_RXRING`
#[inline(always)]
pub unsafe fn netmap_rxring(
    nifp: *mut netmap_if,
    index: usize,
) -> *mut netmap_ring {
    assert!(!nifp.is_null());
    
    let offset = unsafe {
        let ptr = (*nifp)
            .ring_ofs
            .as_ptr()
            .add(index)
            .add((*nifp).ni_tx_rings as _)
            .add((*nifp).ni_host_tx_rings as _);
        *ptr
    };
    unsafe { __netmap_offset!(netmap_ring, nifp, offset) }
}


/// Equivalent to `NETMAP_BUF`
#[inline(always)]
pub fn netmap_buf(ring: &NetmapRing, index: usize) -> *const libc::c_char {
    unsafe {
        (ring.deref() as *const _ as *const libc::c_char)
            .add(ring.buf_ofs as _)
            .add(index * ring.nr_buf_size as usize)
    }
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
