use c_netmap_wrapper::bindings::timeval;

use crate::sockets::PkthdrTrait;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pkthdr {
    pub ts: timeval,
    pub len: u32,
    pub caplen: u32,
    pub buf_idx: u32,
}

impl PkthdrTrait for Pkthdr {
    
}
