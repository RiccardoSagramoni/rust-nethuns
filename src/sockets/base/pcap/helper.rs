use std::cell::RefCell;
use std::rc::Rc;
use std::slice;

use crate::sockets::ring::NethunsRingSlot;


/// TODO
pub unsafe fn get_packet_ref<'a>(
    _rc_slot: &'a Rc<RefCell<NethunsRingSlot>>,
    packet: &[u8],
    len: usize,
) -> &'a [u8] {
    slice::from_raw_parts(packet.as_ptr(), len)
}
