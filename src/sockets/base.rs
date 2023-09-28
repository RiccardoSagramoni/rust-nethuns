pub mod pcap;

use std::cell::RefCell;
use std::ffi::CString;
use std::rc::Rc;
use std::sync::atomic;

use derivative::Derivative;
use ouroboros::self_referencing;

use crate::types::{NethunsQueue, NethunsSocketOptions};

use super::ring::{NethunsRing, NethunsRingSlot};
use super::PkthdrTrait;


/// Closure type for the filtering of received packets. 
/// Returns true if the packet should be received, false if it should be discarded.
type NethunsFilter = dyn Fn(&dyn PkthdrTrait, &[u8]) -> bool;


/// Base structure for a `NethunsSocket`.
///
/// This data structure is common to all the implementation of a "nethuns socket",
/// for the supported underlying I/O frameworks. Thus, it's independent from
/// low-level implementation of the sockets.
#[derive(Default, Derivative)]
#[derivative(Debug)]
pub struct NethunsSocketBase {
    /// Configuration options
    pub opt: NethunsSocketOptions,
    /// Rings used for transmission
    pub tx_ring: Option<NethunsRing>,
    /// Rings used for reception
    pub rx_ring: Option<NethunsRing>,
    /// Name of the binded device
    pub devname: CString,
    /// Queue binded to the socket
    pub queue: NethunsQueue,
    /// Index of the interface
    pub ifindex: libc::c_int,
    /// Closure used for filtering received packets. 
    #[derivative(Debug = "ignore")]
    pub filter: Option<Box<NethunsFilter>>,
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
#[derive(Debug)]
pub struct RecvPacket {
    pub id: usize,
    pub pkthdr: Box<dyn PkthdrTrait>,
    pub packet: RecvPacketData,
}


#[self_referencing(pub_extras)]
#[derive(Debug)]
pub struct RecvPacketData {
    slot: Rc<RefCell<NethunsRingSlot>>,
    #[borrows(slot)]
    pub packet: &'this [u8],
}

impl Drop for RecvPacket {
    /// Release the buffer obtained by calling `recv()`.
    fn drop(&mut self) {
        // Unset the `inuse` flag of the related ring slot
        self.packet.borrow_slot()
            .borrow_mut()
            .inuse
            .store(0, atomic::Ordering::Release);
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
        RecvPacket {
            id,
            pkthdr,
            packet,
        }
    }
}
