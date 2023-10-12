use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

use atomic_enum::atomic_enum;

use crate::sockets::Pkthdr;


#[derive(PartialEq, PartialOrd, Eq, Ord)]
#[atomic_enum]
pub enum InUseStatus {
    Free,
    Reading,
    Sending,
}


#[derive(Debug)]
pub struct RingSlotMutex {
    /// In-use flag => `0`: not in use; `1`: in use (a thread is reading a packet); `2`: in-flight (a thread is sending a packet)
    status: AtomicInUseStatus,
    
    inner: UnsafeCell<NethunsRingSlot>,
}


#[derive(Debug)]
pub enum SlotError {
    // TODO thiserror
    NotFree,
}

unsafe impl Send for RingSlotMutex {}

impl RingSlotMutex {
    pub fn new(pktsize: usize) -> Self {
        Self {
            inner: UnsafeCell::new(NethunsRingSlot::default_with_packet_size(
                pktsize,
            )),
            status: AtomicInUseStatus::new(InUseStatus::Free),
        }
    }
    
    pub fn status(&self) -> InUseStatus {
        self.status.load(Ordering::Acquire)
    }
    
    pub fn set_status(&self, status: InUseStatus) {
        self.status.store(status, Ordering::Release)
    }
    
    pub unsafe fn inner(&self) -> &NethunsRingSlot {
        &*self.inner.get()
    }
    
    pub unsafe fn inner_mut(&self) -> &mut NethunsRingSlot {
        &mut *self.inner.get()
    }
}


#[derive(Debug)]
pub struct RingSlotGuard<'a> {
    slot: &'a RingSlotMutex,
}

impl Deref for RingSlotGuard<'_> {
    type Target = NethunsRingSlot;
    
    fn deref(&self) -> &NethunsRingSlot {
        unsafe { &*self.slot.inner.get() }
    }
}

impl DerefMut for RingSlotGuard<'_> {
    fn deref_mut(&mut self) -> &mut NethunsRingSlot {
        unsafe { &mut *self.slot.inner.get() }
    }
}

impl Drop for RingSlotGuard<'_> {
    fn drop(&mut self) {
        self.slot.status.store(InUseStatus::Free, Ordering::Release);
    }
}


///
///
///
///

/// Ring slot of a Nethuns socket.
#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub(crate) pkthdr: Pkthdr,
    pub(crate) id: usize,
    pub(crate) len: usize,
    
    pub(crate) packet: Vec<u8>,
}


impl NethunsRingSlot {
    /// Get a new `NethunsRingSlot` with `packet` initialized
    /// with a given packet size.
    pub fn default_with_packet_size(pktsize: usize) -> Self {
        NethunsRingSlot {
            packet: vec![0; pktsize],
            ..Default::default()
        }
    }
}
