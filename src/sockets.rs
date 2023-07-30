pub mod base;
pub mod ring;
pub mod types;
pub mod errors;

cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        mod netmap;
        pub use netmap::*;
    } else {
        std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
    }
}
