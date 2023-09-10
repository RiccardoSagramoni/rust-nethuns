pub mod base;
pub mod errors;
pub mod ring;


use core::fmt::Debug;
use std::ffi::CStr;

use crate::types::{NethunsQueue, NethunsSocketOptions, NethunsStat};

use self::base::{NethunsSocketBase, RecvPacket};
use self::errors::{
    NethunsBindError, NethunsFlushError, NethunsOpenError, NethunsRecvError,
    NethunsSendError,
};


/*
    Import the structs defined for the required I/O framework.
    
    Every framework-specific implementation must provide:
    - A struct which implements the `BindableNethunsSocket` trait.
    - A struct which implements the `NethunsSocket` trait.
    - A struct named `Pkthdr` which must implement the `PkthdrTrait` trait.
    TODO: move it to mod documentation
*/
cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        mod netmap;
        
        use netmap::bindable_socket::BindableNethunsSocketNetmap;
        
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
/// * `Ok(Box<dyn BindableNethunsSocket>)` - A new nethuns socket, in no error occurs.
/// * `Err(NethunsOpenError::InvalidOptions)` - If at least one of the options holds a invalid value.
/// * `Err(NethunsOpenError::Error)` - If an unexpected error occurs.
pub fn nethuns_socket_open(
    opt: NethunsSocketOptions,
) -> Result<Box<dyn BindableNethunsSocket>, NethunsOpenError> {
    cfg_if::cfg_if! {
        if #[cfg(feature="netmap")] {
            return BindableNethunsSocketNetmap::open(opt);
        }
        else {
            std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
        }
    }
}


/// Trait which defines the interface for a Nethuns socket
/// not binded to a specific device and queue.
///
/// In order to properly use the socket, you need to bind it first
/// to a specific device and queue by calling [`BindableNethunsSocket::bind()`].
pub trait BindableNethunsSocket: Debug {
    /// Bind an opened socket to a specific queue / any queue of interface/device `dev`.
    ///
    /// # Returns
    /// * `Ok(())` - If the binding was successful.
    /// * `Err(NethunsBindError::IllegalArgument)` - If the device name contains an interior null character.
    /// * `Err(NethunsBindError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsBindError::Error)` - If an unexpected error occurs.
    fn bind(
        self: Box<Self>,
        dev: &str,
        queue: NethunsQueue,
    ) -> Result<
        Box<dyn NethunsSocket>,
        (NethunsBindError, Box<dyn BindableNethunsSocket>),
    >;
    
    /// Get an immutable reference to the base descriptor of the socket.
    fn base(&self) -> &NethunsSocketBase;
    /// Get an mutable reference to the base descriptor of the socket.
    fn base_mut(&mut self) -> &mut NethunsSocketBase;
    
    /// Check if the socket is in RX mode
    #[inline(always)]
    fn rx(&self) -> bool {
        self.base().rx_ring.is_some()
    }
    
    /// Check if the socket is in TX mode
    #[inline(always)]
    fn tx(&self) -> bool {
        self.base().tx_ring.is_some()
    }
}


/// Trait which defines the interface for a Nethuns socket after binding.
/// This socket is usable for RX and/or TX, depending from its configuration.
pub trait NethunsSocket: Debug {
    /// Get the next unprocessed received packet.
    ///
    /// # Returns
    /// * `Ok(RecvPacket)` - The unprocessed received packet, if no error occurred.
    /// * `Err(NethunsRecvError::NotRx)` -  If the socket is not configured in RX mode. Check the configuration parameters passed to `open(...)`.
    /// * `Err(NethunsRecvError::InUse)` - If the slot at the head of the RX ring is currently in use, i.e. the corresponding received packet is not released yet.
    /// * `Err(NethunsRecvError::NoPacketsAvailable)` - If there are no new packets available in the RX ring.
    /// * `Err(NethunsRecvError::PacketFiltered)` - If the packet is filtered out by the `filter` function specified during socket configuration. TODO improve
    /// * `Err(NethunsRecvError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsRecvError::Error)` - If an unexpected error occurs.
    fn recv(&mut self) -> Result<RecvPacket, NethunsRecvError>;
    
    
    /// Queue up a packet for transmission.
    ///
    /// # Returns
    /// * `Ok(())` - On success.
    /// * `Err(NethunsSendError::NotTx)` -  If the socket is not configured in TX mode. Check the configuration parameters passed to `open(...)`.
    /// * `Err(NethunsSendError::InUse)` - If the slot at the tail of the TX ring is not released yet and it's currently in use by the application.
    fn send(&mut self, packet: &[u8]) -> Result<(), NethunsSendError>;
    
    
    /// Send all queued up packets.
    ///
    /// # Returns
    /// * `Ok(())` - On success.
    /// * `Err(NethunsFlushError::NotTx)` -  If the socket is not configured in TX mode. Check the configuration parameters passed to `open(...)`.
    /// * `Err(NethunsFlushError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsFlushError::Error)` - If an unexpected error occurs.
    fn flush(&mut self) -> Result<(), NethunsFlushError>;
    
    
    /// Mark the packet contained in the a specific slot
    /// of the TX ring as *ready for transmission*.
    ///
    /// # Arguments
    /// * `id` - The id of the slot which contains the packet to send.
    /// * `len` - The length of the packet.
    ///
    /// # Returns
    /// * `Ok(())` - On success.
    /// * `Err(NethunsSendError::InUse)` - If the slot is not released yet and it's currently in use by the application.
    fn send_slot(&self, id: usize, len: usize) -> Result<(), NethunsSendError>;
    
    
    /// Get an immutable reference to the base socket descriptor.
    fn base(&self) -> &NethunsSocketBase;
    /// Get a mutable reference to the base socket descriptor.
    fn base_mut(&mut self) -> &mut NethunsSocketBase;
    
    
    /// Check if the socket is in TX mode
    #[inline(always)]
    fn tx(&self) -> bool {
        self.base().tx_ring.is_some()
    }
    
    /// Check if the socket is in RX mode
    #[inline(always)]
    fn rx(&self) -> bool {
        self.base().rx_ring.is_some()
    }
    
    
    /// Get the file descriptor of the socket.
    fn fd(&self) -> libc::c_int;
    
    
    /// Get a mutable reference to the buffer inside
    /// a specific ring slot which will contain the packet
    /// to be sent.
    ///
    /// Equivalent to `nethuns_get_buf_addr` in the C API.
    ///
    /// # Arguments
    /// * `pktid` - id of the slot.
    ///
    /// # Returns
    /// * `Some(&mut [u8])` - buffer reference.
    /// * `None` - if the socket is not in TX mode.
    fn get_packet_buffer_ref(&self, pktid: usize) -> Option<&mut [u8]>;
    
    
    /// Join a fanout group.
    ///
    /// # Arguments
    /// * `group` - The group id.
    /// * `fanout` - A string encoding the details of the fanout mode.
    fn fanout(&mut self, group: libc::c_int, fanout: &CStr) -> bool;
    
    
    /// Dump the rings.
    fn dump_rings(&mut self);
    
    /// Get some statistics about the socket
    /// or `None` on error.
    fn stats(&self) -> Option<NethunsStat>;
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
                assert!(
                    is_trait!(
                        super::netmap::nethuns_socket::NethunsSocketNetmap,
                        super::NethunsSocket
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
        assert!(is_trait!(super::Pkthdr, super::PkthdrTrait));
    }
}
