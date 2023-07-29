use std::mem;

use crate::sockets::base::NethunsRingSlot;


#[derive(Debug, PartialEq, PartialOrd)] // TODO impl Drop trait
pub struct NethunsRing {
    pub size: usize,
    pub pktsize: usize,
    
    pub head: u64,
    pub tail: u64,
    
    pub mask: usize,
    pub shift: usize,
    
    pub ring_slot: Vec<NethunsRingSlot>,
}


impl NethunsRing {
	
	/// Equivalent to nethuns_make_ring
	pub fn new (nslots: usize, pktsize: usize) -> NethunsRing {
		let ns = nethuns_lpow2(nslots);
		let ss = nethuns_lpow2(mem::size_of::<NethunsRingSlot>() + pktsize);
		
		NethunsRing {
			size: nslots,
			pktsize,
			head: 0,
			tail: 0,
			ring_slot: vec![NethunsRingSlot::default(); ns * ss],
			mask: ns - 1,
			shift: ss.trailing_zeros() as usize
		}
	}
	
	#[inline(always)]
	pub fn get_slot(n: usize) {
		
	}
	
}


/// Compute the closest power of 2 larger or equal than x
#[inline(always)]
pub fn nethuns_lpow2(x: usize) -> usize {
	if x == 0 {
		0 // TODO is it ok?
	}
	else if (x & (x - 1)) == 0 {
		x
	}
	else {
		1 << (mem::size_of::<usize>() * 8 - x.leading_zeros() as usize)
	}
}


#[cfg(test)]
mod tests {
	#[test]
	fn lpow2() {
		assert_eq!(
			super::nethuns_lpow2(0),
			0
		);
		assert_eq!(
			super::nethuns_lpow2(1),
			1
		);
		assert_eq!(
			super::nethuns_lpow2(2),
			2
		);
		assert_eq!(
			super::nethuns_lpow2(30),
			32
		);
	}
}
