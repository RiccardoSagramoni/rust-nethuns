use core::slice;
use std::ffi::CString;
use std::fmt::{self, Debug, Display};
use std::marker::PhantomData;
use std::sync::atomic;

use derivative::Derivative;
use getset::{CopyGetters, Getters, Setters};
use nethuns_hybrid_rc::HybridRc;
use nethuns_hybrid_rc::state_trait::RcState;

use crate::types::{NethunsFilter, NethunsQueue, NethunsSocketOptions};

use super::api::Pkthdr;
use super::ring::{AtomicRingSlotStatus, NethunsRing, RingSlotStatus};
use super::PkthdrTrait;


/// Base structure for a `NethunsSocket`.
///
/// This data structure is common to all the implementation of a "nethuns socket",
/// for the supported underlying I/O frameworks. Thus, it's independent from
/// low-level implementation of the sockets.
#[derive(Derivative, Getters, Setters, CopyGetters)]
#[derivative(Debug)]
#[getset(get = "pub")]
pub struct NethunsSocketBase<State: RcState> {
    /// Configuration options
    pub(super) opt: NethunsSocketOptions,
    
    /// Rings used for transmission
    pub(super) tx_ring: Option<NethunsRing<State>>,
    
    /// Rings used for reception
    pub(super) rx_ring: Option<NethunsRing<State>>,
    
    /// Name of the binded device
    pub(super) devname: CString,
    
    /// Queue binded to the socket
    #[getset(get_copy = "pub with_prefix")]
    pub(super) queue: NethunsQueue,
    
    /// Index of the interface
    #[getset(get_copy = "pub with_prefix")]
    pub(super) ifindex: libc::c_int,
    
    /// Closure used for filtering received packets.
    #[derivative(Debug = "ignore")]
    #[getset(set = "pub")]
    pub(super) filter: Option<Box<NethunsFilter>>,
}
// errbuf removed => use Result as return type
// filter_ctx removed => use closures with move semantics

impl<State: RcState> Default for NethunsSocketBase<State> {
    fn default() -> NethunsSocketBase<State> {
        NethunsSocketBase {
            opt: Default::default(),
            tx_ring: None,
            rx_ring: None,
            devname: Default::default(),
            queue: Default::default(),
            ifindex: Default::default(),
            filter: None,
        }
    }
}

/// Packet received when calling [`NethunsSocket::recv()`](crate::sockets::NethunsSocket::recv)
/// or [`NethunsSocketPcap::read()`](crate::sockets::pcap::NethunsSocketPcap::read).
///
/// The struct contains a [`PhantomData`] marker associated with the socket itself,
/// so that the `RecvPacket` item is valid as long as the socket is alive.
#[derive(Debug)]
pub struct RecvPacket<'a, T, State: RcState> {
    data: RecvPacketData<State>,
    
    phantom_data: PhantomData<&'a T>,
}

/// # Safety
///
/// The `packet` raw pointer is valid as long as the `RecvPacket`
/// item is valid and the library guarantees that we are the only
/// holders of such pointer for the lifetime of the `RecvPacket` item.
/// Thus, it can be safely send between threads.
unsafe impl<T: Send, State: RcState> Send for RecvPacket<'_, T, State> {}

impl<'a, T, State: RcState> RecvPacket<'a, T, State> {
    pub(super) fn new(
        data: RecvPacketData<State>,
        phantom_data: PhantomData<&'a T>,
    ) -> Self {
        RecvPacket { data, phantom_data }
    }
    
    #[inline(always)]
    pub fn id(&self) -> usize {
        self.data.id
    }
    
    #[inline(always)]
    pub fn pkthdr(&self) -> &dyn PkthdrTrait {
        // [SAFETY]: the `self.data.pkthdr` raw pointer points to
        // a field to the socket which the current `RecvPacket` is bound to.
        unsafe { &*self.data.pkthdr }
    }
    
    #[inline(always)]
    pub fn buffer(&self) -> &[u8] {
        // [SAFETY]: the `self.data.buffer_ptr` raw pointer points to a buffer
        // inside the socket which the current `RecvPacket` is bound to.
        unsafe {
            slice::from_raw_parts(self.data.buffer_ptr, self.data.buffer_len)
        }
    }
}

impl<T, State: RcState> Display for RecvPacket<'_, T, State> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{\n    id: {},\n    pkthdr: {:?},\n    buffer: {:?}\n}}",
            self.id(),
            self.pkthdr(),
            self.buffer()
        )
    }
}


/// Packet received when calling [`NethunsSocket::recv()`](crate::sockets::NethunsSocket::recv)
/// or [`NethunsSocketPcap::read()`](crate::sockets::pcap::NethunsSocketPcap::read)
/// with static lifetime.
///
/// It **must** be wrapped inside `RecvPacket` struct before being handed to the user.
#[derive(Debug)]
pub(super) struct RecvPacketData<State: RcState> {
    id: usize,
    pkthdr: *const dyn PkthdrTrait,
    
    buffer_ptr: *const u8,
    buffer_len: usize,
    
    slot_status_flag: HybridRc<AtomicRingSlotStatus, State>,
}

impl<State: RcState> RecvPacketData<State> {
    pub fn new(
        id: usize,
        pkthdr: &Pkthdr,
        buffer: &[u8],
        slot_status_flag: HybridRc<AtomicRingSlotStatus, State>,
    ) -> Self {
        Self {
            id,
            pkthdr: pkthdr as *const Pkthdr,
            buffer_ptr: buffer.as_ptr(),
            buffer_len: buffer.len(),
            slot_status_flag,
        }
    }
}

impl<State: RcState> Drop for RecvPacketData<State> {
    /// Release the buffer obtained by calling `recv()`.
    fn drop(&mut self) {
        // Unset the `inuse` flag of the related ring slot
        self.slot_status_flag
            .store(RingSlotStatus::Free, atomic::Ordering::Release);
    }
}
