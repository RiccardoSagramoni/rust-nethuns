pub mod pcap;

use std::ffi::CString;
use std::fmt::{self, Debug, Display};

use derivative::Derivative;
use getset::{CopyGetters, Getters, Setters};
use ouroboros::self_referencing;

use crate::misc::send_rc::SendRc;
use crate::types::{NethunsQueue, NethunsSocketOptions};

use super::ring::{
    InUseStatus, NethunsRing, RingSlotMutex,
};
use super::PkthdrTrait;


/// Closure type for the filtering of received packets.
/// Returns true if the packet should be received, false if it should be discarded.
type NethunsFilter = dyn Fn(&dyn PkthdrTrait, &[u8]) -> bool + Send;


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
#[getset(get = "pub")]
pub struct RecvPacket {
    id: usize,
    pkthdr: Box<dyn PkthdrTrait>,
    packet: RecvPacketData,
}

impl Display for RecvPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{\n    id: {},\n    pkthdr: {:?},\n    packet: {}\n}}",
            self.id, self.pkthdr, self.packet
        )
    }
}


#[self_referencing(pub_extras)]
#[derive(Debug)]
pub struct RecvPacketData {
    slot: SendRc<RingSlotMutex>,
    #[borrows(slot)]
    pub packet: &'this [u8],
}

impl Display for RecvPacketData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.with(|safe_self| write!(f, "{:?}", &safe_self.packet))
    }
}

impl Drop for RecvPacket {
    /// Release the buffer obtained by calling `recv()`.
    fn drop(&mut self) {
        // Unset the `inuse` flag of the related ring slot
        self.packet.borrow_slot().set_status(InUseStatus::Free)
    }
}


impl RecvPacket {
    /// Create a new `RecvPacket` instance.
    ///
    /// # Arguments
    ///
    /// - `id`: The ID of the received packet.
    /// - `pkthdr`: A boxed trait object representing packet header metadata.
    /// - `packet`: A byte slice containing the received packet.
    /// - `slot`: A weak reference to the Nethuns ring slot where the packet is stored. This is required to automatically release the packet once it goes out of scope.
    pub fn new(
        id: usize,
        pkthdr: Box<dyn PkthdrTrait>,
        packet: RecvPacketData,
    ) -> RecvPacket {
        RecvPacket { id, pkthdr, packet }
    }
}
