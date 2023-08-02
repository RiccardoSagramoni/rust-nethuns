use std::ffi::CString;

use c_netmap_wrapper::bindings::__IncompleteArrayField;
use derivative::Derivative;
use libc::c_void;

use crate::types::{NethunsSocketOptions, NethunsQueue};

use super::Pkthdr;
use super::ring::NethunsRing;
use super::types::NethunsPkthdrType;


#[repr(C)]
#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub pkthdr: Pkthdr,
    pub id: u64,
    pub inuse: libc::c_int,
    pub len: i32,
    
    pub packet: __IncompleteArrayField<libc::c_uchar>,
}


#[repr(C)]
#[derive(Debug, Derivative, PartialEq, PartialOrd)]
#[derivative(Default)]
pub struct NethunsSocketBase {
    #[derivative(Default(value = "[0; 512]"))]
    pub errbuf: [libc::c_char; 512],
    
    pub opt: NethunsSocketOptions,
    pub tx_ring: Option<NethunsRing>,
    pub rx_ring: Option<NethunsRing>,
    pub devname: CString,
    pub queue: NethunsQueue,
    pub ifindex: libc::c_int,
    
    // pub filter: Option<fn(*const c_void, &NethunsPkthdrType, &[u8]) -> i32>, /* TODO what type for this closure? */
    pub filter: u64,
    #[derivative(Default(value = "std::ptr::null()"))]
    pub filter_ctx: *const libc::c_void, // TODO: void* ??????
}


// TODO continue
