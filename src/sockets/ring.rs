use std::mem;
use std::sync::Mutex;

use super::ring_slot::NethunsRingSlot;


#[repr(C)]
#[derive(Debug)]
pub struct NethunsRing {
    pub size: usize,
    pub pktsize: usize,
    
    pub head: u64,
    pub tail: u64,
    
    ring: Vec<Mutex<NethunsRingSlot>>,
}


impl NethunsRing {
    /// Equivalent to nethuns_make_ring
    #[inline(always)]
    pub fn try_new(
        nslots: usize,
        pktsize: usize,
    ) -> Result<NethunsRing, String> {
        let mut rings = Vec::with_capacity(nslots);
        for i in 0..nslots {
            rings.push(Mutex::new(NethunsRingSlot::default_with_packet_size(
                pktsize,
            )));
        }
        
        Ok(NethunsRing {
            size: nslots,
            pktsize,
            head: 0,
            tail: 0,
            ring: rings,
        })
    }
    
    /// Equivalent to nethuns_get_slot
    #[inline(always)]
    pub fn get_slot(
        self: &NethunsRing,
        n: usize,
    ) -> &Mutex<NethunsRingSlot> {
        let n = n % self.ring.len();
        &(self.ring[n])
    }
    
    /// Equivalent to nethuns_get_slot
    #[inline(always)]
    pub fn get_slot_mut(
        self: &mut NethunsRing,
        n: usize,
    ) -> &mut Mutex<NethunsRingSlot> {
        let n = n % self.ring.len();
        &mut (self.ring[n])
    }
}


/// Compute the closest power of 2 larger or equal than x
#[inline(always)]
pub fn nethuns_lpow2(x: usize) -> usize { // TODO move to another module?
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
