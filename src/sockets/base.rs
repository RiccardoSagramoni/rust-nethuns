use std::ffi::CString;

use c_netmap_wrapper::bindings::__IncompleteArrayField;
use derivative::Derivative;

use crate::types::{NethunsSocketOptions, NethunsQueue};

use super::Pkthdr;
use super::ring::NethunsRing;


#[repr(C)] // FIXME necessary?
#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub pkthdr: Pkthdr, // FIXME is it ok?
    pub id: u64,
    pub inuse: libc::c_int,
    pub len: i32,
    
    pub packet: __IncompleteArrayField<libc::c_uchar>,
}


#[repr(C)] // FIXME: necessary?
#[derive(Debug, Derivative, PartialEq, PartialOrd)]
#[derivative(Default)]
pub struct NethunsSocketBase {
    #[derivative(Default(value = "[0; 512]"))]
    pub errbuf: [libc::c_char; 512], // FIXME: necessary?
    
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
