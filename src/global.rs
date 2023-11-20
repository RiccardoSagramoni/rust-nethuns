//! Global state of devices

use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Mutex;

use once_cell::sync::Lazy;


/// Struct which holds networking information
/// relative to a unique device.
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)] // disable warnings due to missing implementation of XDP
pub struct NethunsNetInfo {
    pub promisc_refcnt: i32,
    /// xdp only
    pub xdp_prog_refcnt: i32,
    /// xdp only
    pub xdp_prog_id: u32,
}


/// Networking information of all the available Nethuns-enabled devices.
/// Global R/W allowed in a thread-safe manner (mutex).
pub static NETHUNS_GLOBAL: Mutex<
    Lazy<HashMap<CString, NethunsNetInfo, ahash::RandomState>>,
> = Mutex::new(Lazy::new(
    || HashMap::with_hasher(ahash::RandomState::new()),
));
