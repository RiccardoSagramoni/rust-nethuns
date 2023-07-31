use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Mutex;

use once_cell::sync::Lazy;


#[derive(Clone, Copy, Debug, Default)]
pub struct NethunsNetInfo {
    pub promisc_refcnt: u32,
    pub xdp_prog_refcnt: u32,
    pub xdp_prog_id: u32,
}

pub static NETHUNS_GLOBAL: Mutex<Lazy<HashMap<CString, NethunsNetInfo>>> =
    Mutex::new(Lazy::new(|| HashMap::new()));
