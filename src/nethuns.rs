use std::ffi::{CStr, CString};

use crate::NethunsSocket;


///
pub fn __nethuns_set_if_promisc(
    s: &impl NethunsSocket,
    devname: &CStr,
) -> Result<(), String> {
	
	
	
    todo!()
}
