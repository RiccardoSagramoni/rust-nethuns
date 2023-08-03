use c_netmap_wrapper::bindings::timeval;

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pkthdr {
    pub ts: timeval,
    pub len: u32,
    pub caplen: u32,
    pub buf_idx: u32,
}
