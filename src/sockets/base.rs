use std::ffi::CString;

use libc::c_void;

use crate::types::{NethunsSocketOptions, NethunsQueue};

use super::Pkthdr;
use super::ring::NethunsRing;
use super::types::NethunsPkthdrType;


#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct NethunsRingSlot {
    pub pkthdr: Pkthdr,
    pub id: u64,
    pub inuse: i32, // TODO bool?
    pub len: i32,
    
    pub packet: Option<String>, // TODO check best type
}


#[derive(Debug, Default, PartialEq, PartialOrd)]
pub struct NethunsSocketBase {
    pub errbuf: String, // TODO is it necessary? Check usage
    
    pub opt: NethunsSocketOptions,
    pub tx_ring: Option<NethunsRing>,
    pub rx_ring: Option<NethunsRing>,
    pub devname: CString,
    pub queue: NethunsQueue,
    pub ifindex: i32,
    
    pub filter: Option<fn(*const c_void, &NethunsPkthdrType, &[u8]) -> i32>, /* TODO what type for this closure? */
    pub filter_ctx: (), // TODO: void* ??????
}


// TODO continue
