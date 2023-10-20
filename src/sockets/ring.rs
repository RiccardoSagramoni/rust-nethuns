use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::{cmp, ptr};

use getset::{Getters, MutGetters};

use super::api::Pkthdr;
use super::NethunsSocket;

use crate::misc::circular_buffer::CircularBuffer;


/// Ring abstraction for Nethuns sockets.
#[derive(Debug, Getters, MutGetters)]
pub struct NethunsRing {
    #[getset(get = "pub")]
    pktsize: usize,
    
    #[getset(get = "pub(crate)", get_mut = "pub(crate)")]
    rings: CircularBuffer<NethunsRingSlot>,
}


impl NethunsRing {
    /// Create a new `NethunsRing` object.
    ///
    /// Equivalent to `nethuns_make_ring` from the original C library.
    #[inline(always)]
    pub fn new(nslots: usize, pktsize: usize) -> NethunsRing {
        let builder = || NethunsRingSlot::default_with_packet_size(pktsize);
        
        NethunsRing {
            pktsize,
            rings: CircularBuffer::new(nslots, &builder),
        }
    }
    
    
    /// Get a reference to a slot in the ring, given its index.
    #[inline(always)]
    pub fn get_slot(&self, index: usize) -> &NethunsRingSlot {
        self.rings.get(index)
    }
    
    /// Get a reference to a slot in the ring, given its index.
    #[inline(always)]
    pub fn get_slot_mut(&mut self, index: usize) -> &mut NethunsRingSlot {
        self.rings.get_mut(index)
    }
    
    
    /// Get the index of a slot in the ring, given its reference.
    #[inline(always)]
    pub fn get_idx_slot(&self, slot: &NethunsRingSlot) -> Option<usize> {
        // FIXME: this is inefficient. Can we improve it?
        self.rings
            .iter()
            .take(self.rings.size())
            .position(|s| ptr::eq(s, slot))
    }
    
    
    /// Get the number of slots in the ring.
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.rings.size()
    }
    
    /// Check if the buffer is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.rings.is_empty()
    }
    
    /// Check if the buffer is full
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.rings.is_full()
    }
    
    /// Get the current head index
    #[inline(always)]
    pub fn head(&self) -> usize {
        self.rings.head()
    }
    
    /// Get the current tail index
    #[inline(always)]
    pub fn tail(&self) -> usize {
        self.rings.tail()
    }
    
    
    /// Get the number of the consecutive available slots
    /// in the ring, starting from the given position.
    ///
    /// The returned value is capped to 32.
    #[inline(always)]
    pub fn num_free_slots(&self, pos: usize) -> usize {
        let mut total = 0_usize;
        
        for slot in self
            .rings
            .iter()
            .skip(pos)
            .take(cmp::min(self.size() - 1, 32))
        {
            if slot.inuse.load(Ordering::Acquire) == RingSlotStatus::Free {
                total += 1;
            } else {
                break;
            }
        }
        
        total
    }
    
    
    /// Get a reference to the head slot in the ring
    /// and shift the head to the following slot.
    pub fn next_slot(&mut self) -> &NethunsRingSlot {
        self.rings.pop_unchecked()
    }
    
    
    /// Mark the packet contained in a specific slot of a TX ring
    /// as *ready for transmission*, by setting to 1 the `inuse` field.
    ///
    /// # Arguments
    /// * `id` - The id of the slot which contains the packet to send.
    /// * `len` - The length of the packet.
    ///
    /// # Returns
    /// * `true` - On success.
    /// * `false` - If the slot is already in use.
    #[inline(always)]
    pub fn nethuns_send_slot(&mut self, id: usize, len: usize) -> bool {
        let slot = self.get_slot_mut(id as _);
        if slot.inuse.load(Ordering::Acquire) != RingSlotStatus::Free {
            return false;
        }
        slot.len = len;
        slot.inuse.store(RingSlotStatus::InUse, Ordering::Release);
        true
    }
}


/// Ring slot of a Nethuns socket.
#[derive(Debug, Default)]
pub struct NethunsRingSlot {
    pub(crate) inuse: Arc<AtomicRingSlotStatus>,
    
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


/// Status of a ring slot
#[derive(Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub enum RingSlotStatus {
    /// Not in use
    #[default]
    Free,
    /// In-use (a thread is reading a packet from the slot or the slot is marked for sending)
    InUse,
    /// In-flight (the slot is in the middle of a flushing operation)
    InFlight,
}


/// A wrapper around [`RingSlotStatus`] which can be safely shared between threads.
///
/// This type uses an [`AtomicU8`] to store the enum value.
pub struct AtomicRingSlotStatus(AtomicU8);

impl AtomicRingSlotStatus {
    const fn to_u8(val: RingSlotStatus) -> u8 {
        val as u8
    }
    
    fn from_u8(val: u8) -> RingSlotStatus {
        #![allow(non_upper_case_globals)]
        const U8_Free: u8 = RingSlotStatus::Free as u8;
        const U8_InUse: u8 = RingSlotStatus::InUse as u8;
        const U8_InFlight: u8 = RingSlotStatus::InFlight as u8;
        
        match val {
            U8_Free => RingSlotStatus::Free,
            U8_InUse => RingSlotStatus::InUse,
            U8_InFlight => RingSlotStatus::InFlight,
            _ => panic!("Invalid enum discriminant"),
        }
    }
    
    /// Creates a new atomic [`RingSlotStatus`].
    pub const fn new(v: RingSlotStatus) -> AtomicRingSlotStatus {
        AtomicRingSlotStatus(AtomicU8::new(Self::to_u8(v)))
    }
    
    /// Consumes the atomic and returns the contained value.
    ///
    /// This is safe because passing self by value guarantees that
    /// no other threads are concurrently accessing the atomic data.
    pub fn into_inner(self) -> RingSlotStatus {
        Self::from_u8(self.0.into_inner())
    }
    
    /// Loads a value from the atomic.
    ///
    /// `load` takes an `Ordering` argument which describes the memory ordering of this operation. Possiblvalues are `SeqCst`, `Acquire` and `Relaxed`."]
    ///
    /// # Panics
    ///
    /// Panics if order is `Release` or `AcqRel`.
    pub fn load(&self, order: Ordering) -> RingSlotStatus {
        Self::from_u8(self.0.load(order))
    }
    
    /// Stores a value into the atomic.
    ///
    /// `store` takes an `Ordering` argument which describes the memory ordering of this operation. Possible values are `SeqCst`, `Release` and `Relaxed`.
    ///
    /// # Panics
    /// Panics if order is `Acquire` or `AcqRel`.
    pub fn store(&self, val: RingSlotStatus, order: Ordering) {
        self.0.store(Self::to_u8(val), order)
    }
}

impl From<RingSlotStatus> for AtomicRingSlotStatus {
    fn from(val: RingSlotStatus) -> AtomicRingSlotStatus {
        AtomicRingSlotStatus::new(val)
    }
}
impl core::fmt::Debug for AtomicRingSlotStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        self.load(Ordering::SeqCst).fmt(f)
    }
}

impl Default for AtomicRingSlotStatus {
    fn default() -> Self {
        Self::new(RingSlotStatus::default())
    }
}


/// Free all the currently unused slots in the ring.
///
/// # Arguments
/// * `socket` - A reference to the `NethunsSocket` object.
/// * `ring` - A reference to the `NethunsRing` object.
/// * `free_macro` - The name of the macro to call to free the slots. It must exposed the following interface: `free_macro(socket, slot, block_id)`
macro_rules! nethuns_ring_free_slots {
    ($socket: expr, $ring: expr, $free_macro: ident) => {
        loop {
            let slot = $ring.get_slot($ring.rings().tail());
            
            if $ring.rings().is_empty()
                || slot.inuse.load(Ordering::Acquire)
                    != crate::sockets::ring::RingSlotStatus::Free
            {
                break;
            }
            
            $free_macro!($socket, slot, slot.id);
            $ring.rings_mut().advance_tail();
        }
    };
}
pub(super) use nethuns_ring_free_slots;


/// Get size of the RX ring.
#[inline(always)]
pub fn rxring_get_size(socket: &NethunsSocket) -> Option<usize> {
    socket.base().rx_ring().as_ref().map(|r| r.size())
}


/// Get size of the TX ring.
#[inline(always)]
pub fn txring_get_size(socket: &NethunsSocket) -> Option<usize> {
    socket.base().tx_ring().as_ref().map(|r| r.size())
}
