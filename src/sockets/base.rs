use std::ffi::CString;

use derive_builder::Builder;
use libc::c_void;

use crate::types::NethunsSocketOptions;

use super::types::NethunsPkthdrType;


// TODO
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct NethunsRingSlot();


#[derive(Clone, Builder, Debug, Default, PartialEq, PartialOrd)]
#[builder(pattern = "owned", default)]
pub struct NethunsRing {
    pub size: usize,
    pub pktsize: usize,
    
    pub head: u64,
    pub tail: u64,
    
    pub mask: usize,
    pub shift: usize,
    
    pub ring: NethunsRingSlot,
}


#[derive(Clone, Builder, Debug, Default, PartialEq, PartialOrd)]
#[builder(pattern = "owned", default)]
pub struct NethunsSocketBase {
    pub errbuf: String, // TODO is it necessary? Check usage
    
    pub opt: NethunsSocketOptions,
    pub tx_ring: NethunsRing,
    pub rx_ring: NethunsRing,
    pub devname: CString,
    pub queue: i32,
    pub ifindex: i32,
    
    pub filter: Option<fn(*const c_void, &NethunsPkthdrType, &[u8]) -> i32>, /* TODO what type for this closure? */
    pub filter_ctx: (), // TODO: void* ??????
}


// TODO continue
