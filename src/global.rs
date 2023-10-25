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


/// Set RLIMIT_MEMLOCK to infinity at application startup.
///
/// This function requires CAP_SYS_RESOURCE capability
/// because the call to [`libc::setrlimit`]
/// (see [setrlimit(2) - Linux man page](https://linux.die.net/man/2/setrlimit)
/// for more details).
/// Since this usually means that we must run the tests with root privileges,
/// this function is disabled while testing.
#[cfg(target_os = "linux")]
#[cfg(not(test))]
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
