use std::ffi::CString;

use libc::c_void;

use crate::types::NethunsSocketOptions;

use super::ring::NethunsRing;
use super::types::NethunsPkthdrType;


// TODO
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct NethunsRingSlot();


#[derive(Debug, Default, PartialEq, PartialOrd)]
pub struct NethunsSocketBase {
    pub errbuf: String, // TODO is it necessary? Check usage
    
    pub opt: NethunsSocketOptions,
    pub tx_ring: Option<NethunsRing>,
    pub rx_ring: Option<NethunsRing>,
    pub devname: CString,
    pub queue: i32,
    pub ifindex: i32,
    
    pub filter: Option<fn(*const c_void, &NethunsPkthdrType, &[u8]) -> i32>, /* TODO what type for this closure? */
    pub filter_ctx: (), // TODO: void* ??????
}



// TODO continue
