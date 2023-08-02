use std::mem;

use super::ring_slot::NethunsRingSlot;


#[repr(C)]
#[derive(Debug, PartialEq, PartialOrd)]
pub struct NethunsRing {
    pub size: usize,
    pub pktsize: usize,
    
    pub head: u64,
    pub tail: u64,
    
    pub mask: usize,
    pub shift: usize,
    
    pub ring: *mut NethunsRingSlot,
}


impl NethunsRing {
    /// Equivalent to nethuns_make_ring
    #[inline(always)]
    pub fn try_new(
        nslots: usize,
        pktsize: usize,
    ) -> Result<NethunsRing, String> {
        let ns = nethuns_lpow2(nslots);
        let ss = nethuns_lpow2(mem::size_of::<NethunsRingSlot>() + pktsize);
        
        let ring_ptr =
            unsafe { libc::calloc(1, ns * ss) as *mut NethunsRingSlot };
        
        if ring_ptr.is_null() {
            return Err(
                "[NethunsRing::try_new] failed to allocate ring".to_owned()
            );
        }
        
        Ok(NethunsRing {
            size: nslots,
            pktsize,
            head: 0,
            tail: 0,
            ring: ring_ptr,
            mask: ns - 1,
            shift: ss.trailing_zeros() as usize,
        })
    }
    
    /// Equivalent to nethuns_get_slot
    #[inline(always)]
    pub fn get_slot(self: &NethunsRing, n: usize) -> &mut NethunsRingSlot {
        assert!(!self.ring.is_null());
        
        unsafe {
            &mut *((self.ring as *const libc::c_char)
                .add((n & self.mask) << self.shift)
                as *mut NethunsRingSlot)
        }
    }
}


impl Drop for NethunsRing {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.ring as *mut libc::c_void);
            libc::memset(
                self as *mut NethunsRing as *mut libc::c_void,
                0,
                mem::size_of::<NethunsRing>(),
            ); // ? necessary?
        }
    }
}


/// Compute the closest power of 2 larger or equal than x
#[inline(always)]
pub fn nethuns_lpow2(x: usize) -> usize {
    if x == 0 {
        0 // FIXME is it ok?
    } else if (x & (x - 1)) == 0 {
        x
    } else {
        1 << (mem::size_of::<usize>() * 8 - x.leading_zeros() as usize)
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn lpow2() {
        assert_eq!(super::nethuns_lpow2(0), 0);
        assert_eq!(super::nethuns_lpow2(1), 1);
        assert_eq!(super::nethuns_lpow2(2), 2);
        assert_eq!(super::nethuns_lpow2(30), 32);
    }
}
