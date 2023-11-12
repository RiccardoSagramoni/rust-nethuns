mod api;
pub mod base;
pub mod errors;
pub mod pcap;
mod ring;

pub use api::PkthdrTrait;


use core::fmt::Debug;
use std::cell::UnsafeCell;
use std::ffi::CStr;
use std::marker::PhantomData;

use crate::misc::hybrid_rc::state_trait::RcState;
use crate::types::{
    NethunsFilter, NethunsQueue, NethunsSocketOptions, NethunsStat,
};

use self::api::{
    BindableNethunsSocketInner, BindableNethunsSocketInnerTrait,
    LocalRxNethunsSocketTrait, NethunsSocketInner, NethunsSocketInnerTrait,
    SharedRxNethunsSocketTrait,
};
use self::base::{NSRecvPacket, NethunsSocketBase, RecvPacket};
use self::errors::{
    NethunsBindError, NethunsFlushError, NethunsOpenError, NethunsRecvError,
    NethunsSendError,
};

/// Mark [`BindableNethunsSocket`], [`NethunsSocket`] or [`RecvPacket`] as local,
/// i.e. as referenceable from a single thread.
///
/// Derived from the [`HybridRc`](https://docs.rs/hybrid-rc/) crate.
pub type Local = crate::misc::hybrid_rc::state::Local;

/// Mark [`BindableNethunsSocket`], [`NethunsSocket`] or [`RecvPacket`] as shared,
/// i.e. as referenceable from multiple threads.
///
/// Derived from the [`HybridRc`](https://docs.rs/hybrid-rc/) crate.
pub type Shared = crate::misc::hybrid_rc::state::Shared;


/// Type for a Nethuns socket not binded to a specific device and queue.
///
///
/// In order to properly use the socket, you need to bind it first
/// to a specific device and queue by calling [`BindableNethunsSocket::bind`]
#[derive(Debug)]
#[repr(transparent)]
pub struct BindableNethunsSocket<State: RcState> {
    /// Framework-specific socket
    inner: Box<BindableNethunsSocketInner<State>>,
}

// Make sure BindableNethunsSocket is Send
static_assertions::assert_impl_all!(BindableNethunsSocket<Shared>: Send);

impl<State: RcState> BindableNethunsSocket<State> {
    /// Open a new Nethuns socket, by calling the `open` function
    /// of the struct belonging to the I/O framework selected at compile time.
    ///
    /// # Arguments
    /// * `opt`: The options for the socket.
    ///
    /// # Returns
    /// * `Ok(BindableNethunsSocket)` - A new nethuns socket, in no error occurs.
    /// * `Err(NethunsOpenError::InvalidOptions)` - If at least one of the options holds a invalid value.
    /// * `Err(NethunsOpenError::Error)` - If an unexpected error occurs.
    pub fn open(opt: NethunsSocketOptions) -> Result<Self, NethunsOpenError> {
        api::nethuns_socket_open(opt).map(|inner| Self {
            inner: Box::new(inner),
        })
    }
    
    /// Bind an opened socket to a specific queue / any queue of interface/device `dev`.
    ///
    /// # Returns
    /// * `Ok(())` - If the binding was successful.
    /// * `Err(NethunsBindError::IllegalArgument)` - If the device name contains an interior null character.
    /// * `Err(NethunsBindError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsBindError::Error)` - If an unexpected error occurs.
    pub fn bind(
        self,
        dev: &str,
        queue: NethunsQueue,
    ) -> Result<NethunsSocket<State>, (NethunsBindError, Self)> {
        self.inner
            .bind(dev, queue)
            .map_err(|(error, socket)| (error, Self { inner: socket }))
    }
    
    delegate::delegate! {
        to self.inner {
            /// Check if the socket is in RX mode
            #[inline(always)]
            pub fn rx(&self) -> bool;
            
            /// Check if the socket is in TX mode
            #[inline(always)]
            pub fn tx(&self) -> bool;
        }
    }
}


/// Descriptor of a Nethuns socket after binding.
///
/// This socket is usable for RX and/or TX, depending from its configuration.
#[derive(Debug)]
#[repr(transparent)]
pub struct NethunsSocket<State: RcState> {
    inner: UnsafeCell<NethunsSocketInner<State>>,
}

// Make sure BindableNethunsSocket is Send
static_assertions::assert_impl_all!(NethunsSocket<Shared>: Send);

impl<State: RcState> NethunsSocket<State> {
    /// Create a new `NethunsSocket`.
    fn new(inner: NethunsSocketInner<State>) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
        }
    }
    
    
    /// Queue up a packet for transmission.
    ///
    /// # Returns
    /// * `Ok(())` - On success.
    /// * `Err(NethunsSendError::NotTx)` -  If the socket is not configured in TX mode. Check the configuration parameters passed to [`BindableNethunsSocket::open`].
    /// * `Err(NethunsSendError::InUse)` - If the slot at the tail of the TX ring is not released yet and it's currently in use by the application.
    pub fn send(&self, packet: &[u8]) -> Result<(), NethunsSendError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).send(packet) }
    }
    
    
    /// Send all queued up packets.
    ///
    /// # Returns
    /// * `Ok(())` - On success.
    /// * `Err(NethunsFlushError::NotTx)` -  If the socket is not configured in TX mode. Check the configuration parameters passed to [`BindableNethunsSocket::open`].
    /// * `Err(NethunsFlushError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsFlushError::Error)` - If an unexpected error occurs.
    pub fn flush(&self) -> Result<(), NethunsFlushError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).flush() }
    }
    
    
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
    pub fn send_slot(
        &self,
        id: usize,
        len: usize,
    ) -> Result<(), NethunsSendError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).send_slot(id, len) }
    }
    
    
    /// Set the optional packet filtering function.
    ///
    /// # Parameters
    /// * `filter` - The packet filtering function. `None` if no filtering is required, `Some(filter)` to enable packet filtering.
    #[inline(always)]
    pub fn set_filter(&self, filter: Option<Box<NethunsFilter>>) {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).base_mut() }
            .set_filter(filter);
    }
    
    
    /// Get the file descriptor of the socket.
    pub fn fd(&self) -> std::os::raw::c_int {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).fd() }
    }
    
    
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
    pub fn get_packet_buffer_ref(&mut self, pktid: usize) -> Option<&mut [u8]> {
        // Enforce unique access to the socket, since we are modifying a packet buffer
        UnsafeCell::get_mut(&mut self.inner).get_packet_buffer_ref(pktid)
    }
    
    
    /// Join a fanout group.
    ///
    /// # Arguments
    /// * `group` - The group id.
    /// * `fanout` - A string encoding the details of the fanout mode.
    pub fn fanout(&self, group: i32, fanout: &CStr) -> bool {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).fanout(group, fanout) }
    }
    
    
    /// Dump the rings of the socket.
    pub fn dump_rings(&self) {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).dump_rings() }
    }
    
    /// Get some statistics about the socket
    /// or `None` on error.
    pub fn stats(&self) -> Option<NethunsStat> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).stats() }
    }
    
    
    #[inline(always)]
    pub(crate) fn base(&self) -> &NethunsSocketBase<State> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).base() }
    }
    
    /// Check if the socket is in TX mode
    #[inline(always)]
    pub fn tx(&self) -> bool {
        self.base().tx_ring().is_some()
    }
    
    /// Check if the socket is in RX mode
    #[inline(always)]
    pub fn rx(&self) -> bool {
        self.base().rx_ring().is_some()
    }
    
    /// Get size of the RX ring.
    #[inline(always)]
    pub fn rxring_get_size(&self) -> Option<usize> {
        self.base().rx_ring().as_ref().map(|r| r.size())
    }
    
    /// Get size of the TX ring.
    #[inline(always)]
    pub fn txring_get_size(&self) -> Option<usize> {
        self.base().tx_ring().as_ref().map(|r| r.size())
    }
}

impl NethunsSocket<Local> {
    /// Get the next unprocessed received packet.
    ///
    /// # Returns
    /// * `Ok(RecvPacket<NethunsSocket>)` - The unprocessed received packet, if no error occurred.
    /// * `Err(NethunsRecvError::NotRx)` -  If the socket is not configured in RX mode. Check the configuration parameters passed to [`BindableNethunsSocket::open`].
    /// * `Err(NethunsRecvError::InUse)` - If the slot at the head of the RX ring is currently in use, i.e. the corresponding received packet is not released yet.
    /// * `Err(NethunsRecvError::NoPacketsAvailable)` - If there are no new packets available in the RX ring.
    /// * `Err(NethunsRecvError::PacketFiltered)` - If the packet is filtered out by the `filter` function specified during socket configuration.
    /// * `Err(NethunsRecvError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsRecvError::Error)` - If an unexpected error occurs.
    pub fn recv(&self) -> Result<NSRecvPacket<Local, Local>, NethunsRecvError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).recv() }
            .map(|data| RecvPacket::new(data, PhantomData))
    }
}

impl NethunsSocket<Shared> {
    /// Get the next unprocessed received packet.
    ///
    /// # Returns
    /// * `Ok(RecvPacket<NethunsSocket>)` - The unprocessed received packet, if no error occurred.
    /// * `Err(NethunsRecvError::NotRx)` -  If the socket is not configured in RX mode. Check the configuration parameters passed to [`BindableNethunsSocket::open`].
    /// * `Err(NethunsRecvError::InUse)` - If the slot at the head of the RX ring is currently in use, i.e. the corresponding received packet is not released yet.
    /// * `Err(NethunsRecvError::NoPacketsAvailable)` - If there are no new packets available in the RX ring.
    /// * `Err(NethunsRecvError::PacketFiltered)` - If the packet is filtered out by the `filter` function specified during socket configuration.
    /// * `Err(NethunsRecvError::FrameworkError)` - If an error from the unsafe interaction with underlying I/O framework occurs.
    /// * `Err(NethunsRecvError::Error)` - If an unexpected error occurs.
    pub fn recv(
        &self,
    ) -> Result<NSRecvPacket<Shared, Shared>, NethunsRecvError> {
        unsafe { (*UnsafeCell::raw_get(&self.inner)).recv() }
            .map(|data| RecvPacket::new(data, PhantomData))
    }
}
