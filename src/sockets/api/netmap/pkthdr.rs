//! Packet header for netmap framework

use c_netmap_wrapper::bindings::timeval;

use crate::sockets::PkthdrTrait;


/// Packet header containing metadata
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PkthdrNetmap {
    pub ts: timeval,
    pub len: u32,
    pub caplen: u32,
    pub buf_idx: u32,
}


impl PkthdrTrait for PkthdrNetmap {
    #[inline(always)]
    fn tstamp_sec(&self) -> u32 {
        self.ts.tv_sec as _
    }
    #[inline(always)]
    fn tstamp_usec(&self) -> u32 {
        self.ts.tv_usec as _
    }
    #[inline(always)]
    fn tstamp_nsec(&self) -> u32 {
        (self.ts.tv_usec * 1000) as _
    }
    #[inline(always)]
    fn tstamp_set_sec(&mut self, sec: u32) {
        self.ts.tv_sec = sec as _;
    }
    #[inline(always)]
    fn tstamp_set_usec(&mut self, usec: u32) {
        self.ts.tv_usec = usec as _;
    }
    #[inline(always)]
    fn tstamp_set_nsec(&mut self, nsec: u32) {
        self.ts.tv_usec = (nsec / 1000) as _;
    }
    #[inline(always)]
    fn snaplen(&self) -> u32 {
        self.caplen
    }
    #[inline(always)]
    fn len(&self) -> u32 {
        self.len
    }
    #[inline(always)]
    fn set_snaplen(&mut self, len: u32) {
        self.caplen = len
    }
    #[inline(always)]
    fn set_len(&mut self, len: u32) {
        self.len = len
    }
    #[inline(always)]
    fn rxhash(&self) -> u32 {
        0
    }
    #[inline(always)]
    fn offvlan_tpid(&self) -> u16 {
        0
    }
    #[inline(always)]
    fn offvlan_tci(&self) -> u16 {
        0
    }
}
