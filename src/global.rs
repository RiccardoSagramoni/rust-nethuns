use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Mutex;

use once_cell::sync::Lazy;


/// Struct which holds networking information 
/// relative to a unique device.
#[derive(Clone, Copy, Debug, Default)]
pub struct NethunsNetInfo {
    pub promisc_refcnt: i32,
    pub xdp_prog_refcnt: u32,
    pub xdp_prog_id: u32,
}

/// Networking information of all the available Nethuns-enabled devices.
/// Global R/W allowed in a thread-safe manner (mutex).
pub static NETHUNS_GLOBAL: Mutex<Lazy<HashMap<CString, NethunsNetInfo>>> =
    Mutex::new(Lazy::new(HashMap::new));
