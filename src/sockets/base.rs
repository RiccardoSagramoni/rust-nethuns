pub mod pcap;

use std::ffi::CString;
use std::fmt::{self, Debug, Display};
use std::marker::PhantomData;
use std::sync::{atomic, Arc};

use derivative::Derivative;
use getset::{CopyGetters, Getters, Setters};

use crate::types::{NethunsQueue, NethunsSocketOptions, NethunsFilter};

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


/// Packet received when calling `recv()` on a `NethunsSocket` object.
///
/// It's valid as long as the related `NethunsRingSlot` object is alive.
///
/// You can use the `RecvPacket::try_new()` method to create a new instance.
///
/// # Fields
/// - `id`: the id of the packet.
/// - `pkthdr`: the packet header metadata. Its internal format depends on the selected I/O framework.
/// - `packet`: the Ethernet packet payload.
#[derive(Debug, Getters)]
pub struct RecvPacket<'a, T> {
    data: RecvPacketData,
    
    phantom_data: PhantomData<&'a T>,
}

unsafe impl<T: Send> Send for RecvPacket<'_, T> {}

impl<T> RecvPacket<'_, T> {
    /// Create a new `RecvPacket` instance.
    ///
    /// TODO
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
