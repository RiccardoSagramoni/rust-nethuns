#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use libc::c_ulong;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

unsafe impl Send for nethuns_socket_options {}
unsafe impl Sync for nethuns_socket_options {}


pub fn nethuns_rx_release(sock: *mut nethuns_socket_netmap, pkt_id: c_ulong) {
    if sock.is_null() {
        return;
    }
    
    unsafe {
        let mut nethuns_ring = (*(sock as *mut nethuns_socket_base)).rx_ring;
        let nethuns_ring_slot =
            nethuns_ring_get_slot(&mut nethuns_ring, (pkt_id - 1) as usize);
        (*nethuns_ring_slot).inuse = 0;
    }
}
