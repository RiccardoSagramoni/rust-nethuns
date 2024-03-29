//! Common structures for all the implementation of a Nethuns socket.

use std::ffi::CString;
use std::fmt::{self, Debug, Display};
use std::sync::atomic;

use derivative::Derivative;

use crate::types::{NethunsFilter, NethunsQueue, NethunsSocketOptions};

use super::ring::{AtomicRingSlotStatus, NethunsRing, RingSlotStatus};
use super::PkthdrTrait;


/// Base structure for a `NethunsSocket`.
///
/// This data structure is common to all the implementation of a "nethuns socket",
/// for the supported underlying I/O frameworks. Thus, it's independent from
/// low-level implementation of the sockets.
#[derive(Default, Derivative)]
#[derivative(Debug)]
pub(crate) struct NethunsSocketBase {
    /// Configuration options
    pub opt: NethunsSocketOptions,
    
    /// Ring used for transmission
    pub tx_ring: Option<NethunsRing>,
    
    /// Ring used for reception
    pub rx_ring: Option<NethunsRing>,
    
    /// Name of the binded device
    pub devname: CString,
    
    /// Queue binded to the socket
    pub queue: NethunsQueue,
    
    /// Index of the interface
    pub ifindex: i32,
    
    /// Closure used for filtering received packets.
    #[derivative(Debug = "ignore")]
    pub filter: Option<Box<NethunsFilter>>,
}
// errbuf removed => use Result as return type
// filter_ctx removed => use closures with move semantics


//


/// Public data structure for a packet received when calling [`NethunsSocket::recv()`](crate::sockets::NethunsSocket::recv) or [`NethunsSocketPcap::read()`](crate::sockets::pcap::NethunsSocketPcap::read).
///
/// The lifetime specifier is required to ensure that the references do not outlive the generating socket.
#[derive(Debug)]
pub struct RecvPacket<'a> {
    id: usize,
    pkthdr: &'a dyn PkthdrTrait,
    buffer: &'a [u8],
    /// Reference used to set the status flag of the corresponding ring slot
    /// to `Free` when the `RecPacketData` is dropped.
    slot_status_flag: &'a AtomicRingSlotStatus,
}


impl<'a> RecvPacket<'a> {
    pub(super) fn new(
        id: usize,
        pkthdr: &'a dyn PkthdrTrait,
        buffer: &'a [u8],
        slot_status_flag: &'a AtomicRingSlotStatus,
    ) -> Self {
        Self {
            id,
            pkthdr,
            buffer,
            slot_status_flag,
        }
    }
    
    #[inline(always)]
    pub fn id(&self) -> usize {
        self.id
    }
    
    #[inline(always)]
    pub fn pkthdr(&self) -> &dyn PkthdrTrait {
        self.pkthdr
    }
    
    #[inline(always)]
    pub fn buffer(&self) -> &[u8] {
        self.buffer
    }
}


impl Display for RecvPacket<'_> {
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

impl Drop for RecvPacket<'_> {
    /// Release the buffer by resetting the status flag of
    /// the corresponding ring slot.
    fn drop(&mut self) {
        self.slot_status_flag
            .store(RingSlotStatus::Free, atomic::Ordering::Release);
    }
}
