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


/// Set RLIMIT_MEMLOCK to infinity at application startup.
#[cfg(target_os = "linux")]
#[small_ctor::ctor]
unsafe fn setrlimit() {
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim);
    if ret != 0 {
        libc::fprintf(
            libc::fdopen(libc::STDERR_FILENO, "w+".as_ptr() as _) as _,
            "nethuns: setrlimit(RLIMIT_MEMLOCK) \"%s\"\n".as_ptr() as _,
            libc::strerror(*libc::__errno_location()),
        );
        libc::exit(1);
    }
}
