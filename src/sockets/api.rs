//! This module exposes the data structures required
//! to interact with the specified I/O framework.
//!
//! Every framework-specific implementation must provide:
//! - A struct which implements the [`BindableNethunsSocketTrait`] trait.
//! - A struct which implements the [`NethunsSocketTrait`](super::NethunsSocketTrait) trait.
//! - A struct named [`Pkthdr`] which must implement the [`PkthdrTrait`](super::PkthdrTrait) trait.

use crate::types::NethunsSocketOptions;

use super::errors::NethunsOpenError;
use super::BindableNethunsSocketTrait;

cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        mod netmap;
        
        pub use netmap::pkthdr::Pkthdr;
    }
    else {
        std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
    }
}


/// Open a new Nethuns socket, by calling the `open` function
/// of the struct belonging to the I/O framework selected at compile time.
///
/// # Arguments
/// * `opt`: The options for the socket.
///
/// # Returns
/// * `Ok(Box<dyn BindableNethunsSocketTrait>)` - A new nethuns socket, in no error occurs.
/// * `Err(NethunsOpenError::InvalidOptions)` - If at least one of the options holds a invalid value.
/// * `Err(NethunsOpenError::Error)` - If an unexpected error occurs.
pub fn nethuns_socket_open(
    opt: NethunsSocketOptions,
) -> Result<Box<dyn BindableNethunsSocketTrait>, NethunsOpenError> {
    cfg_if::cfg_if! {
        if #[cfg(feature="netmap")] {
            netmap::bindable_socket::BindableNethunsSocketNetmap::open(opt)
        }
        else {
            std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
        }
    }
}


#[cfg(test)]
mod test {
    use is_trait::is_trait;
    
    use crate::sockets::{NethunsSocketTrait, PkthdrTrait};
    
    use super::*;
    
    #[test]
    /// Make sure that the NethunsSocket trait is implemented correctly.
    fn assert_nethuns_socket_trait() {
        cfg_if::cfg_if! {
            if #[cfg(feature="netmap")] {
                assert!(
                    is_trait!(
                        netmap::bindable_socket::BindableNethunsSocketNetmap,
                        BindableNethunsSocketTrait
                    )
                );
                assert!(
                    is_trait!(
                        netmap::nethuns_socket::NethunsSocketNetmap,
                        NethunsSocketTrait
                    )
                );
            }
            else {
                std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
            }
        }
    }
    
    #[test]
    /// Make sure that the Pkthdr struct implements the PkthdrTrait trait.
    fn assert_pkthdr_trait() {
        assert!(is_trait!(Pkthdr, PkthdrTrait));
    }
}
