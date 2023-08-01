// TODO check safety of raw pointers

use crate::bindings::{netmap_if, netmap_ring};

/// Equivalent to __NETMAP_OFFSET
/// ((type)(void *)((char *)(ptr) + (offset)))
macro_rules! __netmap_offset {
    ($type: ident, $ptr: expr, $off: expr) => {
        unsafe {
            ($ptr as *const libc::c_char).add($off as usize)
                as *const libc::c_void as *mut $type
        }
    };
}
pub(crate) use __netmap_offset;


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
        *ring_ofs_ptr
    };
    __netmap_offset!(netmap_ring, nifp, offset)
}


/// Equivalent to NETMAP_RXRING
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
            .add((*nifp).ni_tx_rings as usize)
            .add((*nifp).ni_host_tx_rings as usize);
        *ptr
    };
    __netmap_offset!(netmap_ring, nifp, offset)
}


/// Equivalent to C macro NETMAP_BUF(ring, index)
#[inline(always)]
pub fn netmap_buf(ring: &netmap_ring, index: usize) -> *const libc::c_char {
    unsafe {
        (ring as *const _ as *const libc::c_char)
            .add(ring.buf_ofs as usize)
            .add(index * ring.nr_buf_size as usize)
    }
}


#[cfg(test)]
mod test {
    use std::mem::size_of;
    
    #[test]
    fn test_netmap_offset() {
        let x = 0;
        let x_ptr = &x as *const i32 as *mut i32;
        
        assert_eq!(__netmap_offset!(i32, x_ptr, 0), x_ptr);
        
        assert_eq!(__netmap_offset!(i32, x_ptr, size_of::<i32>()), unsafe {
            x_ptr.add(1)
        });
        
        assert_eq!(
            __netmap_offset!(i32, x_ptr, 10 * size_of::<i32>()),
            unsafe { x_ptr.add(10) }
        );
    }
}
