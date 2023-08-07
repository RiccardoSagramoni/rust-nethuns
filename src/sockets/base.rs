use std::cell::RefCell;
use std::ffi::CString;
use std::rc::Weak;
use std::sync::atomic;

use derivative::Derivative;
use etherparse::PacketHeaders;

use crate::types::{NethunsQueue, NethunsSocketOptions};

use super::ring::NethunsRing;
use super::ring_slot::NethunsRingSlot;
use super::PkthdrTrait;


/// Closure type for the filtering of received packets
type NethunsFilter = dyn Fn(&dyn PkthdrTrait, &[u8]) -> i32;


/// Base structure for a `NethunsSocket`.
/// 
/// This data structure is common to all the implementation of a "nethuns socket",
/// for the supported underlying I/O frameworks. Thus, it's indipendent from
/// low-level implementation of the sockets.
#[derive(Derivative)]
#[derivative(Debug, Default)]
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
    /// Closure used for filtering received packets
    #[derivative(Debug = "ignore")]
    pub filter: Option<Box<NethunsFilter>>,
}
// errbuf removed => use Result as return type
// filter_ctx removed => use closures with move semantics


/// Packet received when calling `recv()` on a `NethunsSocket` object.
///
/// You can use the `RecvPacket::try_new()` method to create a new instance.
///
/// # Fields
/// - `id`: the id of the packet.
/// - `pkthdr`: the packet header metadata. Its internal format depends on the selected I/O framework
/// - `packet`: the Ethernet packet payload, deserialized.
#[derive(Debug)]
pub struct RecvPacket<'a> {
    pub id: u64,
    pub pkthdr: Box<dyn PkthdrTrait>,
    pub packet: PacketHeaders<'a>,
    
    slot: Weak<RefCell<NethunsRingSlot>>,
}

impl Drop for RecvPacket<'_> {
    /// Release the buffer obtained by calling `recv()`.
    fn drop(&mut self) {
        if let Some(rc) = self.slot.upgrade() {
            // Unset the `inuse` flag of the related ring slot
            rc.borrow_mut()
                .inuse
                .store(false, atomic::Ordering::Release);
        }
    }
}

impl RecvPacket<'_> {
    /// Tries to create a new `RecvPacket` instance.
    ///
    /// # Arguments
    ///
    /// - `id`: The ID of the received packet.
    /// - `pkthdr`: A boxed trait object representing packet header metadata.
    /// - `pkt`: A byte slice containing the received packet.
    /// - `slot`: A weak reference to the Nethuns ring slot where the packet is stored.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing either the created `RecvPacket` instance on success,
    /// or an `etherparse::ReadError` if there was an error parsing the packet headers.
    pub fn try_new(
        id: u64,
        pkthdr: Box<dyn PkthdrTrait>,
        pkt: &'_ [u8],
        slot: Weak<RefCell<NethunsRingSlot>>,
    ) -> Result<RecvPacket<'_>, etherparse::ReadError> {
        Ok(RecvPacket {
            id,
            pkthdr,
            packet: PacketHeaders::from_ethernet_slice(pkt)?,
            slot,
        })
    }
}


// TODO continue
