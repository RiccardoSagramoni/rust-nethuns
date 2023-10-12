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


/// Slot of a socket ring.
///
/// It holds a status flag which indicates if the slot is currently "in use" by a thread
/// and provide mutable access to its inner data with dynamically checked borrow rules.
#[derive(Debug)]
pub struct NethunsRingSlot {
    status: AtomicInUseStatus,
    
    inner: RefCell<NethunsRingSlotInner>, // RefCell guarantees more safetu against UB
}

unsafe impl Send for NethunsRingSlot {}

impl NethunsRingSlot {
    /// Create a new `NethunsRingSlot` object with an allocated
    /// buffer of size `pktsize`.
    pub fn new(pktsize: usize) -> Self {
        Self {
            inner: RefCell::new(
                NethunsRingSlotInner::default_with_packet_size(pktsize),
            ),
            status: AtomicInUseStatus::new(InUseStatus::Free),
        }
    }
    
    /// Get the current status flag in a thread-safe manner.
    pub fn status(&self) -> InUseStatus {
        self.status.load(Ordering::Acquire)
    }
    
    /// Set the current status flag in a thread-safe manner.
    pub fn set_status(&self, status: InUseStatus) {
        self.status.store(status, Ordering::Release)
    }
    
    /// Immutably borrows the inner structure.
    pub fn borrow(&self) -> Ref<'_, NethunsRingSlotInner> {
        self.inner.borrow()
    }
    
    /// Mutably borrows the inner structure.
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
