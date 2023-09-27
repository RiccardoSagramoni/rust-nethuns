use std::cell::RefCell;
use std::rc::Rc;
use std::slice;

use crate::sockets::ring::NethunsRingSlot;

pub fn get_packet_ref<'a>(
    _rc_slot: &'a Rc<RefCell<NethunsRingSlot>>,
    packet: &[u8],
    len: usize,
) -> &'a [u8] {
    unsafe { slice::from_raw_parts(packet.as_ptr(), len) }
}
