use std::cell::RefCell;
use std::ffi::CString;
use std::rc::Weak;
use std::sync::atomic;

use derivative::Derivative;
use etherparse::{SlicedPacket, PacketHeaders};

use crate::types::{NethunsQueue, NethunsSocketOptions};

use super::ring::NethunsRing;
use super::ring_slot::NethunsRingSlot;
use super::PkthdrTrait;

type NethunsFilter = dyn Fn(&dyn PkthdrTrait, &[u8]) -> i32;


#[derive(Derivative)]
#[derivative(Debug, Default)]
pub struct NethunsSocketBase {
    pub opt: NethunsSocketOptions,
    pub tx_ring: Option<NethunsRing>,
    pub rx_ring: Option<NethunsRing>,
    pub devname: CString,
    pub queue: NethunsQueue,
    pub ifindex: libc::c_int,
    
    #[derivative(Debug = "ignore")]
    pub filter: Option<Box<NethunsFilter>>,
}
// errbuf removed => use Result as return type
// filter_ctx removed => use closures with move semantics


///
#[derive(Debug)]
pub struct RecvPacket<'a> {
    pub id: u64,
    pub pkthdr: Box<dyn PkthdrTrait>,
    pub packet: PacketHeaders<'a>,
    
    slot: Weak<RefCell<NethunsRingSlot>>,
}

impl Drop for RecvPacket<'_> {
    fn drop(&mut self) {
        // Release the slot
        if let Some(rc) = self.slot.upgrade() {
            rc.borrow_mut()
                .inuse
                .store(false, atomic::Ordering::Release);
        }
    }
}

impl RecvPacket<'_> {
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
