mod global;

// Nethuns public API {
pub mod misc;
pub mod sockets;
pub mod types;
pub mod vlan;
// }


/// Set `RLIMIT_MEMLOCK` to infinity at application startup.
///
/// This function is automatically called before any application code is execution,
/// thanks to the [small-ctor crate](https://docs.rs/small_ctor/latest/small_ctor/).
///
/// `CAP_SYS_RESOURCE` capability is required to run this function,
/// because of the call to [`libc::setrlimit`]
/// (see [setrlimit(2) - Linux man page](https://linux.die.net/man/2/setrlimit)
/// for more details).
/// Since this would mean that we must run the tests with root privileges,
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
            "nethuns: setrlimit(RLIMIT_MEMLOCK) \"%s\"\n\0".as_ptr() as _,
            libc::strerror(*libc::__errno_location()),
        );
        libc::exit(1);
    }
}
