use core::slice;
use std::ffi::CString;
use std::fmt::{self, Debug, Display};
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::sync::atomic;

use derivative::Derivative;
use getset::{CopyGetters, Getters, Setters};

use crate::misc::hybrid_rc::state::{Local, Shared};
use crate::misc::hybrid_rc::state_trait::RcState;
use crate::misc::hybrid_rc::HybridRc;
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
    // errbuf removed => use Result as return type
    // filter_ctx removed => use closures with move semantics
}

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


//


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
unsafe impl<T> Send for RecvPacket<'_, T, Shared> {}


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


impl<'a, T> RecvPacket<'a, T, Local> {
    /// Convert the local received packet to a shared one,
    /// so that it can be sent between threads.
    pub fn to_shared(mut self) -> RecvPacket<'a, T, Shared> {
        let shared_packet = RecvPacket {
            data: RecvPacketData {
                id: self.data.id,
                pkthdr: self.data.pkthdr,
                buffer_ptr: self.data.buffer_ptr,
                buffer_len: self.data.buffer_len,
                slot_status_flag: ManuallyDrop::new(HybridRc::to_shared(
                    &self.data.slot_status_flag,
                )),
            },
            phantom_data: self.phantom_data,
        };
        mem::drop(unsafe {
            ManuallyDrop::take(&mut self.data.slot_status_flag)
        });
        mem::forget(self);
        shared_packet
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


//


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
    
    /// Reference used to set the status flag of the corresponding ring slot
    /// to `Free` when the `RecPacketData` is dropped.
    ///
    /// The [`ManuallyDrop`] wrapper is required to convert a
    /// [`GenericRecvPacket<'_, T, Local>`] object
    /// to a [`GenericRecvPacket<'_, T, Shared>`] object without
    /// calling the [`drop()`] method (which would reset the status flag).
    slot_status_flag: ManuallyDrop<HybridRc<AtomicRingSlotStatus, State>>,
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
            slot_status_flag: ManuallyDrop::new(slot_status_flag),
        }
    }
}

impl<State: RcState> Drop for RecvPacketData<State> {
    /// Release the buffer by resetting the status flag of
    /// the corresponding ring slot.
    fn drop(&mut self) {
        self.slot_status_flag
            .store(RingSlotStatus::Free, atomic::Ordering::Release);
        
        mem::drop(unsafe { ManuallyDrop::take(&mut self.slot_status_flag) });
    }
}
