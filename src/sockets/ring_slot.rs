use c_netmap_wrapper::bindings::__IncompleteArrayField;

use super::Pkthdr;

#[repr(C)] // ! IMPORTANT: managed by C code in kernel
#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub pkthdr: Pkthdr, // FIXME is it ok?
    pub id: u64,
    pub inuse: libc::c_int,
    pub len: i32,
    
    pub packet: __IncompleteArrayField<libc::c_uchar>,
}
