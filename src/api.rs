cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        pub use crate::sockets::netmap::*;
    } else {
        std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
    }
}
