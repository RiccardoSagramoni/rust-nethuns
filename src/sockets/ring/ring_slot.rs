use std::cell::{Ref, RefCell, RefMut};
use std::sync::atomic::Ordering;

use atomic_enum::atomic_enum;

use crate::sockets::Pkthdr;


/// Status of a ring slot
///
/// - `Free`: not in use
/// - `Reading`: a thread is reading a packet ("in-use")
/// - `Sending`: a thread is sending a packet ("in-flight")
#[derive(PartialEq, PartialOrd, Eq, Ord)]
#[atomic_enum]
pub enum InUseStatus {
    Free,
    Reading,
    Sending,
}


#[derive(Debug)]
pub struct NethunsRingSlot {
    status: AtomicInUseStatus,
    
    inner: RefCell<NethunsRingSlotInner>, // RefCell guarantees more safetu against UB
}


#[derive(Debug, thiserror::Error)]
pub enum SlotError {
    #[error("the slot is inuse")]
    NotFree,
}

unsafe impl Send for NethunsRingSlot {}

impl NethunsRingSlot {
    pub fn new(pktsize: usize) -> Self {
        Self {
            inner: RefCell::new(
                NethunsRingSlotInner::default_with_packet_size(pktsize),
            ),
            status: AtomicInUseStatus::new(InUseStatus::Free),
        }
    }
    
    pub fn status(&self) -> InUseStatus {
        self.status.load(Ordering::Acquire)
    }
    
    pub fn set_status(&self, status: InUseStatus) {
        self.status.store(status, Ordering::Release)
    }
    
    pub fn borrow(&self) -> Ref<'_, NethunsRingSlotInner> {
        self.inner.borrow()
    }
    
    pub fn borrow_mut(&self) -> RefMut<'_, NethunsRingSlotInner> {
        self.inner.borrow_mut()
    }
}


/// Ring slot of a Nethuns socket.
#[derive(Debug, Default)]
pub struct NethunsRingSlotInner {
    pub(crate) pkthdr: Pkthdr,
    pub(crate) id: usize,
    pub(crate) len: usize,
    
    pub(crate) packet: Vec<u8>,
}


impl NethunsRingSlotInner {
    /// Get a new `NethunsRingSlot` with `packet` initialized
    /// with a given packet size.
    pub fn default_with_packet_size(pktsize: usize) -> Self {
        NethunsRingSlotInner {
            packet: vec![0; pktsize],
            ..Default::default()
        }
    }
}
