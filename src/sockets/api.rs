//! This module exposes the data structures required
//! to interact with the specified I/O framework.
//!
//! Every framework-specific implementation must provide:
//! - A struct named [`BindableNethunsSocketInner`] which implements
//! the [`BindableNethunsSocketInnerTrait`] trait.
//! - A struct named [`NethunsSocketInner`] which implements
//! the [`NethunsSocketInnerTrait`] trait.
//! - A struct named [`Pkthdr`] which implements the [`PkthdrTrait`] trait.

use std::ffi::CStr;
use std::fmt::Debug;

use crate::misc::hybrid_rc::state_trait::RcState;
use crate::types::{NethunsQueue, NethunsSocketOptions, NethunsStat};

use super::base::{NethunsSocketBase, RecvPacketData};
use super::errors::{
    NethunsBindError, NethunsFlushError, NethunsOpenError, NethunsRecvError,
    NethunsSendError,
};
use super::{Local, Shared};


cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        mod netmap;
        
        /// Nethuns socket **before** binding to a specific device and queue.
        pub(super) type BindableNethunsSocketInner<State> = netmap::BindableNethunsSocketNetmap<State>;
        /// Nethuns socket **after** binding to a specific device and queue.
        pub(super) type NethunsSocketInner<State> = netmap::NethunsSocketNetmap<State>;
        /// Packet header metadata
        pub(super) type Pkthdr = netmap::PkthdrNetmap;
    }
    else {
        std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
    }
}


//


/// Open a new Nethuns socket, by calling the `open` function
/// of the struct belonging to the I/O framework selected at compile time.
///
/// # Arguments
/// * `opt`: The options for the socket.
///
/// # Returns
/// * `Ok(BindableNethunsSocketInner<State>)` - A new nethuns socket, in no error occurs.
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


//


/// Trait which defines the interface for the framework-specific
/// implementation of a [`BindableNethunsSocketInner`].
pub(super) trait BindableNethunsSocketInnerTrait<State: RcState>:
    Debug
{
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
    ) -> Result<Box<NethunsSocketInner<State>>, (NethunsBindError, Box<Self>)>
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


/// Trait which defines the interface for the framework-specific
/// implementation of a [`NethunsSocketInner`].
pub(super) trait NethunsSocketInnerTrait<State: RcState>: Debug {
    /// Get an immutable reference to the base socket descriptor.
    fn base(&self) -> &NethunsSocketBase<State>;
    /// Get a mutable reference to the base socket descriptor.
    fn base_mut(&mut self) -> &mut NethunsSocketBase<State>;
    
    
    /// Queue up a packet for transmission.
    ///
    /// # Returns
    /// * `Ok(())` - On success.
    /// * `Err(NethunsSendError::NotTx)` -  If the socket is not configured in TX mode. Check the configuration parameters passed to [`BindableNethunsSocket::open`](super::BindableNethunsSocket::open).
    /// * `Err(NethunsSendError::InUse)` - If the slot at the tail of the TX ring is not released yet and it's currently in use by the application.
    fn send(&mut self, packet: &[u8]) -> Result<(), NethunsSendError>;
    
    
    /// Send all queued up packets.
    ///
    /// # Returns
    /// * `Ok(())` - On success.
    /// * `Err(NethunsFlushError::NotTx)` -  If the socket is not configured in TX mode. Check the configuration parameters passed to [`BindableNethunsSocket::open`](super::BindableNethunsSocket::open).
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
    fn send_slot(
        &mut self,
        id: usize,
        len: usize,
    ) -> Result<(), NethunsSendError>;
    
    
    /// Get the file descriptor of the socket.
    fn fd(&self) -> std::os::raw::c_int;
    
    
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
    fn fanout(&mut self, group: i32, fanout: &CStr) -> bool;
    
    
    /// Dump the rings of the socket.
    fn dump_rings(&mut self);
    
    
    /// Get some statistics about the socket
    /// or `None` on error.
    fn stats(&self) -> Option<NethunsStat>;
}

pub(super) trait LocalRxNethunsSocketTrait:
    Debug + NethunsSocketInnerTrait<Local>
{
    fn recv(&mut self) -> Result<RecvPacketData<Local>, NethunsRecvError>;
}

pub(super) trait SharedRxNethunsSocketTrait:
    Debug + NethunsSocketInnerTrait<Shared>
{
    fn recv(&mut self) -> Result<RecvPacketData<Shared>, NethunsRecvError>;
}


//


#[allow(rustdoc::private_intra_doc_links)]
/// Trait for the [`Pkthdr`] struct,
/// which contains the packet header metadata.
#[allow(clippy::len_without_is_empty)]
pub trait PkthdrTrait: Debug + Send + Sync {
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


//


// Check implementation of API traits
static_assertions::assert_impl_all!(
    BindableNethunsSocketInner<Local>: BindableNethunsSocketInnerTrait<Local>
);
static_assertions::assert_impl_all!(
    BindableNethunsSocketInner<Shared>: BindableNethunsSocketInnerTrait<Shared>, Send
);
static_assertions::assert_impl_all!(
    NethunsSocketInner<Local>: NethunsSocketInnerTrait<Local>
);
static_assertions::assert_impl_all!(
    NethunsSocketInner<Shared>: NethunsSocketInnerTrait<Shared>, Send
);
static_assertions::assert_impl_all!(
    Pkthdr: PkthdrTrait
);
