use std::ffi::CString;

use derivative::Derivative;

use crate::types::{NethunsQueue, NethunsSocketOptions};

use super::ring::NethunsRing;


#[repr(C)] // FIXME: necessary?
#[derive(Debug, Derivative)]
#[derivative(Default)]
pub struct NethunsSocketBase {
    #[derivative(Default(value = "[0; 512]"))]
    pub errbuf: [libc::c_char; 512], // FIXME: unused
    
    pub opt: NethunsSocketOptions,
    pub tx_ring: Option<NethunsRing>,
    pub rx_ring: Option<NethunsRing>,
    pub devname: CString,
    pub queue: NethunsQueue,
    pub ifindex: libc::c_int,
    
    pub filter: u64, // TODO: what type? It should be a closure
    #[derivative(Default(value = "std::ptr::null()"))]
    pub filter_ctx: *const libc::c_void, // FIXME: wrapper?
}


// TODO continue
