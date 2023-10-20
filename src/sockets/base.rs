pub mod pcap;

use std::ffi::CString;
use std::fmt::{self, Debug, Display};
use std::marker::PhantomData;
use std::sync::{atomic, Arc};

use derivative::Derivative;
use getset::{CopyGetters, Getters, Setters};

use crate::types::{NethunsFilter, NethunsQueue, NethunsSocketOptions};

use super::ring::{AtomicRingSlotStatus, NethunsRing, RingSlotStatus};
use super::PkthdrTrait;


/// Base structure for a `NethunsSocket`.
///
/// This data structure is common to all the implementation of a "nethuns socket",
/// for the supported underlying I/O frameworks. Thus, it's independent from
/// low-level implementation of the sockets.
#[derive(Default, Derivative, Getters, Setters, CopyGetters)]
#[derivative(Debug)]
#[getset(get = "pub")]
pub struct NethunsSocketBase {
    /// Configuration options
    pub(super) opt: NethunsSocketOptions,
    
    /// Rings used for transmission
    pub(super) tx_ring: Option<NethunsRing>,
    
    /// Rings used for reception
    pub(super) rx_ring: Option<NethunsRing>,
    
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


/// Packet received when calling [`NethunsSocket::recv()`](crate::sockets::NethunsSocket::recv)
/// or [`NethunsSocketPcap::read()`](crate::sockets::base::pcap::NethunsSocketPcap::read).
///
/// The struct contains a [`PhantomData`] marker associated with the socket itself,
/// so that the `RecvPacket` item is valid as long as the socket is alive.
#[derive(Debug, Getters)]
pub struct RecvPacket<'a, T> {
    data: RecvPacketData,
    
    phantom_data: PhantomData<&'a T>,
}

/// [SAFETY]
/// The `packet` raw pointer is valid as long as the `RecvPacket`
/// item is valid and the library guarantees that we are the only
/// holders of such pointer for the lifetime of the `RecvPacket` item.
/// Thus, it can be safely send between threads.
unsafe impl<T: Send> Send for RecvPacket<'_, T> {}

impl<T> RecvPacket<'_, T> {
    pub fn new(
        data: RecvPacketData,
        phantom_data: PhantomData<&'_ T>,
    ) -> RecvPacket<T> {
        RecvPacket { data, phantom_data }
    }
    
    #[inline(always)]
    pub fn id(&self) -> usize {
        self.data.id
    }
    
    #[inline(always)]
    pub fn pkthdr(&self) -> &dyn PkthdrTrait {
        self.data.pkthdr.as_ref()
    }
    
    #[inline(always)]
    pub fn packet(&self) -> &'_ [u8] {
        unsafe { &*self.data.packet }
    }
}

impl<T> Display for RecvPacket<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{\n    id: {},\n    pkthdr: {:?},\n    packet: {:?}\n}}",
            self.id(),
            self.pkthdr(),
            self.packet()
        )
    }
}


/// Packet received when calling [`NethunsSocket::recv()`](crate::sockets::NethunsSocket::recv)
/// or [`NethunsSocketPcap::read()`](crate::sockets::base::pcap::NethunsSocketPcap::read)
/// with static lifetime.
/// 
/// It **must** be wrapped inside `RecvPacket` struct before being handed to the user.
#[derive(Debug)]
pub struct RecvPacketData {
    id: usize,
    pkthdr: Box<dyn PkthdrTrait>,
    packet: *const [u8],
    
    slot_status_flag: Arc<AtomicRingSlotStatus>,
}

impl RecvPacketData {
    pub(super) fn new(
        id: usize,
        pkthdr: Box<dyn PkthdrTrait>,
        packet: *const [u8],
        slot_status_flag: Arc<AtomicRingSlotStatus>,
    ) -> Self {
        Self {
            id,
            pkthdr,
            packet,
            slot_status_flag,
        }
    }
}

impl Drop for RecvPacketData {
    /// Release the buffer obtained by calling `recv()`.
    fn drop(&mut self) {
        // Unset the `inuse` flag of the related ring slot
        self.slot_status_flag
            .store(RingSlotStatus::Free, atomic::Ordering::Release);
    }
}
