use std::ffi::CString;

use derivative::Derivative;

use crate::types::{NethunsQueue, NethunsSocketOptions};

use super::ring::NethunsRing;
use super::Pkthdr;

type NethunsFilter = dyn Fn(&Pkthdr, *const u8) -> i32; // FIXME safe wrapper?

#[repr(C)] // FIXME: necessary?
#[derive(Derivative)]
#[derivative(Debug, Default)]
pub struct NethunsSocketBase {
    #[derivative(Default(value = "[0; 512]"))]
    pub errbuf: [libc::c_char; 512], // FIXME: unused
    
    pub opt: NethunsSocketOptions,
    pub tx_ring: Option<NethunsRing>,
    pub rx_ring: Option<NethunsRing>,
    pub devname: CString,
    pub queue: NethunsQueue,
    pub ifindex: libc::c_int,
    
    #[derivative(Debug = "ignore")]
    pub filter: Option<Box<NethunsFilter>>,
}


// TODO continue
