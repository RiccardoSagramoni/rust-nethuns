pub mod base;
pub mod errors;
pub mod ring;
pub mod ring_slot;


use core::fmt::Debug;

use crate::types::{NethunsQueue, NethunsSocketOptions};

use self::base::{NethunsSocketBase, RecvPacket};
use self::errors::{NethunsBindError, NethunsOpenError, NethunsRecvError, NethunsSendError, NethunsFlushError};


/*
    Import the structs defined for the required I/O framework.
    
    Every framework-specific implementation must provide:
    - A struct which implements the `NethunsSocket` trait, which will
        be built by the `NethunsSocketFactory` factory.
    - A struct named `Pkthdr` which must implement the `PkthdrTrait` trait.
*/
cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        mod netmap;
        
        use netmap::NethunsSocketNetmap;
        
        pub use netmap::Pkthdr;
    }
    else {
        std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
    }
}


/// Trait which defines the public API for Nethuns sockets.
pub trait NethunsSocket: Debug {
    /// Tries to open a new Nethuns socket.
    ///
    /// # Arguments
    /// * `opt`: The options for the socket.
    ///
    /// # Returns
    /// * `Ok(Box<dyn NethunsSocket>)` - A new nethuns socket, if no error occurred.
    /// * `Err(NethunsOpenError::InvalidOptions)` - If at least one of the options holds a invalid value.
    /// * `Err(NethunsOpenError::Error)` - If an unexpected error occurs.
    fn open(
        opt: NethunsSocketOptions,
    ) -> Result<Box<dyn NethunsSocket>, NethunsOpenError>
    where
        Self: Sized;
    
    
    /// Bind an opened socket to a specific queue / any queue of interface/device `dev`.
    ///
    /// # Returns
    /// * `Ok(())` - If the binding was successful.
    /// * `Err(NethunsBindError::IllegalArgument)` - If the device name contains an interior null character.
    /// * `Err(NethunsBindError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsBindError::Error)` - If an unexpected error occurs.
    fn bind(
        &mut self,
        dev: &str,
        queue: NethunsQueue,
    ) -> Result<(), NethunsBindError>;
    
    
    /// Get the next unprocessed received packet.
    ///
    /// # Returns
    /// * `Ok(RecvPacket)` - The unprocessed received packet, if no error occurred.
    /// * `Err(NethunsRecvError::NonBinded)` - If the socket is not bound to a device. Make sure to call `NethunsSocket::bind(...)` first.
    /// * `Err(NethunsRecvError::NotRx)` -  If the socket is not configured in RX mode. Check the configuration parameters passed to `open(...)`.
    /// * `Err(NethunsRecvError::InUse)` - If the slot at the head of the RX ring is currently in use, i.e. the corresponding received packet is not released yet.
    /// * `Err(NethunsRecvError::NoPacketsAvailable)` - If there are no new packets available in the RX ring.
    /// * `Err(NethunsRecvError::PacketFiltered)` - If the packet is filtered out by the `filter` function specified during socket configuration. TODO improve
    /// * `Err(NethunsRecvError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsRecvError::Error)` - If an unexpected error occurs.
    fn recv(&mut self) -> Result<RecvPacket, NethunsRecvError>;
    
    
    /// TODO
    fn send(&mut self, packet: &[u8]) -> Result<(), NethunsSendError>;
    /// TODO
    fn flush(&mut self) -> Result<(), NethunsFlushError>;
    
    /// Get an immutable reference to the base socket descriptor.
    fn socket_base(&self) -> &NethunsSocketBase;
    /// Get a mutable reference to the base socket descriptor.
    fn socket_base_mut(&mut self) -> &mut NethunsSocketBase;
}


/// Factory to build objects which implements the trait NethunsSocket
pub struct NethunsSocketFactory();

impl NethunsSocketFactory {
    /// Tries to open a new Nethuns socket, by calling the `open` function
    /// of the struct belonging to the I/O framework selected at compile time.
    ///
    /// # Arguments
    /// * `opt`: The options for the socket.
    ///
    /// # Returns
    /// * `Ok(Box<dyn NethunsSocket>)` - A new nethuns socket, in no error occurs.
    /// * `Err(NethunsOpenError::InvalidOptions)` - If at least one of the options holds a invalid value.
    /// * `Err(NethunsOpenError::Error)` - If an unexpected error occurs.
    pub fn try_new_nethuns_socket(
        opt: NethunsSocketOptions,
    ) -> Result<Box<dyn NethunsSocket>, NethunsOpenError> {
        cfg_if::cfg_if! {
            if #[cfg(feature="netmap")] {
                return NethunsSocketNetmap::open(opt);
            }
            else {
                std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
            }
        }
    }
}


/// Trait which defines the public API for Pkthdr,
/// which contains the packet header metadata.
#[allow(clippy::len_without_is_empty)]
pub trait PkthdrTrait: Debug {
    fn tstamp_sec(&self) -> u32;
    fn tstamp_usec(&self) -> u32;
    fn tstamp_nsec(&self) -> u32;
    fn tstamp_set_sec(&mut self, sec: u32);
    fn tstamp_set_usec(&mut self, usec: u32);
    fn tstamp_set_nsec(&mut self, nsec: u32);
    fn snaplen(&self) -> u32;
    fn len(&self) -> u32;
    fn set_snaplen(&mut self, len: u32);
    fn set_len(&mut self, len: u32);
    fn rxhash(&self) -> u32;
    fn offvlan_tpid(&self) -> u16;
    fn offvlan_tci(&self) -> u16;
}


#[cfg(test)]
mod test {
    use is_trait::is_trait;
    
    #[test]
    /// Make sure that the NethunsSocket trait is implemented correctly.
    fn assert_nethuns_socket_trait() {
        cfg_if::cfg_if! {
            if #[cfg(feature="netmap")] {
                assert!(is_trait!(super::NethunsSocketNetmap, super::NethunsSocket));
            }
            else {
                std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
            }
        }
    }
    
    #[test]
    /// Make sure that the Pkthdr struct implements the PkthdrTrait trait.
    fn assert_pkthdr_trait() {
        assert!(is_trait!(super::Pkthdr, super::PkthdrTrait));
    }
}
