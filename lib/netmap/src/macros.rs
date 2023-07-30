use crate::bindings::netmap_ring;


/// Equivalent to C macro NETMAP_BUF(ring, index)
#[inline(always)]
pub fn netmap_buf(ring: &netmap_ring, index: usize) -> *const u8 {
    let byte_index = (index * ring.nr_buf_size as usize) + ring.buf_ofs as usize;
	
	unsafe {
		return (ring as *const netmap_ring as *const u8).add(byte_index);
	}
}
