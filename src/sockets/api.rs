//! This module exposes the data structures required
//! to interact with the specified I/O framework.
//!
//! Every framework-specific implementation must provide:
//! - A struct which implements the [`BindableNethunsSocketTrait`] trait.
//! - A struct which implements the [`NethunsSocketTrait`] trait.
//! - A struct named [`Pkthdr`] which must implement the [`PkthdrTrait`] trait.
//!
//!
//! [`BindableNethunsSocketTrait`]: super::BindableNethunsSocketTrait
//! [`NethunsSocketTrait`]: super::NethunsSocketTrait
//! [`PkthdrTrait`]: super::PkthdrTrait
use std::fmt::Debug;

use nethuns_hybrid_rc::state_trait::RcState;

use crate::types::{NethunsQueue, NethunsSocketOptions};

use super::base::NethunsSocketBase;
use super::errors::{NethunsBindError, NethunsOpenError};
use super::NethunsSocket;


cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        mod netmap;
        
        pub type BindableNethunsSocketInner<State> = netmap::BindableNethunsSocketNetmap<State>;
        pub type Pkthdr = netmap::PkthdrNetmap;
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
pub(super) fn nethuns_socket_open<State: RcState>(
    opt: NethunsSocketOptions,
) -> Result<BindableNethunsSocketInner<State>, NethunsOpenError> {
    cfg_if::cfg_if! {
        if #[cfg(feature="netmap")] {
            netmap::BindableNethunsSocketNetmap::open(opt)
        }
        else {
            std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
        }
    }
}


/// Trait which defines the interface for the framework-specific
/// implementation of a [`BindableNethunsSocket`].
pub(super) trait BindableNethunsSocketInnerTrait<State: RcState>: Debug {
    /// Bind an opened socket to a specific queue / any queue of interface/device `dev`.
    ///
    /// # Returns
    /// * `Ok(())` - If the binding was successful.
    /// * `Err(NethunsBindError::IllegalArgument)` - If the device name contains an interior null character.
    /// * `Err(NethunsBindError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsBindError::Error)` - If an unexpected error occurs.
    fn bind(
        self,
        dev: &str,
        queue: NethunsQueue,
    ) -> Result<NethunsSocket<State>, (NethunsBindError, Self)>
    where
        Self: Sized;
    
    /// Get an immutable reference to the base descriptor of the socket.
    fn base(&self) -> &NethunsSocketBase<State>;
    
    /// Check if the socket is in RX mode
    #[inline(always)]
    fn rx(&self) -> bool {
        self.base().rx_ring().is_some()
    }
    
    /// Check if the socket is in TX mode
    #[inline(always)]
    fn tx(&self) -> bool {
        self.base().tx_ring().is_some()
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
                        netmap::BindableNethunsSocketNetmap<crate::sockets::state::Local>,
                        BindableNethunsSocketInnerTrait<crate::sockets::state::Local>
                    )
                );
                assert!(
                    is_trait!(
                        netmap::NethunsSocketNetmap<crate::sockets::state::Local>,
                        NethunsSocketTrait<crate::sockets::state::Local>
                    )
                );
                assert!(
                    is_trait!(
                        netmap::BindableNethunsSocketNetmap<crate::sockets::state::Shared>,
                        BindableNethunsSocketInnerTrait<crate::sockets::state::Shared>
                    )
                );
                assert!(
                    is_trait!(
                        netmap::NethunsSocketNetmap<crate::sockets::state::Shared>,
                        NethunsSocketTrait<crate::sockets::state::Shared>
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
