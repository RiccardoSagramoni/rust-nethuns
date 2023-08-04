use std::cell::RefCell;
use std::ffi::CString;
use std::rc::Weak;
use std::sync::atomic;

use derivative::Derivative;

use crate::types::{NethunsQueue, NethunsSocketOptions};

use super::ring::NethunsRing;
use super::PkthdrTrait;
use super::ring_slot::NethunsRingSlot;

type NethunsFilter = dyn Fn(&dyn PkthdrTrait, *const u8) -> i32; // FIXME safe wrapper for *const u8?

#[repr(C)] // FIXME: necessary?
#[derive(Derivative)]
#[derivative(Debug, Default)]
pub struct NethunsSocketBase {
    #[derivative(Default(value = "[0; 512]"))]
    pub errbuf: [libc::c_char; 512], // FIXME: unused
    
    pub opt: NethunsSocketOptions,
    pub tx_ring: Option<NethunsRing>,
    pub rx_ring: Option<NethunsRing>,
    pub devname: CString,
    pub queue: NethunsQueue,
    pub ifindex: libc::c_int,
    
    #[derivative(Debug = "ignore")]
    pub filter: Option<Box<NethunsFilter>>,
}


///
#[derive(Debug, derive_new::new)]
pub struct RecvPacket {
    // (u64, Pkthdr, *const u8)
    pub id: u64,
    pub pkthdr: Box<dyn PkthdrTrait>,
    pub payload: *const u8, // FIXME safe wrapper?
    slot: Weak<RefCell<NethunsRingSlot>>,
}

impl Drop for RecvPacket {
    fn drop(&mut self) {
        // Release the slot
        if let Some(rc) = self.slot.upgrade() {
            rc.borrow_mut()
                .inuse
                .store(false, atomic::Ordering::Release);
        }
    }
}


// TODO continue
