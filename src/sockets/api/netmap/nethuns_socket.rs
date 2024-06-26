//! [`NethunsSocket`](crate::sockets::NethunsSocket) inner implementation
//! for the netmap framework.

use std::ffi::CStr;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;
use std::{cmp, mem, slice};

use c_netmap_wrapper::bindings::{nm_pkt_copy, NS_BUF_CHANGED};
use c_netmap_wrapper::constants::{NIOCRXSYNC, NIOCTXSYNC};
use c_netmap_wrapper::macros::{netmap_buf, netmap_txring};
use c_netmap_wrapper::{netmap_buf_pkt, NetmapRing, NmPortDescriptor};

use crate::misc::circular_queue::CircularQueue;
use crate::misc::nethuns_clear_if_promisc;
use crate::sockets::api::NethunsSocketInnerTrait;
use crate::sockets::base::{NethunsSocketBase, RecvPacket};
use crate::sockets::errors::{
    NethunsFlushError, NethunsRecvError, NethunsSendError,
};
use crate::sockets::ring::{
    nethuns_ring_free_slots, NethunsRingSlot, RingSlotStatus,
};
use crate::types::NethunsStat;

use super::utility::{
    nethuns_blocks_free, nethuns_get_buf_addr_netmap, non_empty_rx_ring,
};


/// [`NethunsSocket`](crate::sockets::NethunsSocket) inner implementation
/// for the netmap framework.
#[derive(Debug)]
pub struct NethunsSocketNetmap {
    base: NethunsSocketBase,
    
    /// Port descriptor
    p: NmPortDescriptor,
    
    /// Wrapper of a raw pointer to a [netmap_ring](c_netmap_wrapper::bindings::netmap_ring) object
    /// allocated by the kernel in the userspace.
    /// This is required to know the address of the ring buffer.
    some_ring: NetmapRing,
    
    /// Circular array of available buffers for I/O.
    ///
    /// When a netmap port is opened, its `netmap_rings` are already filled
    /// with a buffer in each netmap_slot, but it is possible to request
    /// that other "free" buffers not already associated with netmap_slots
    /// be allocated as well.
    /// Our library allocates these free buffers and places them in the [`free_ring`](Self::free_ring).
    /// On the receiving end, new packets are written by the network interface
    /// into the buffers associated with the netmap_slots;
    /// [`recv()`](NethunsSocketNetmap::recv()) extracts one of these buffers to pass it to
    /// the user, and it puts a new buffer extracted from the free_ring
    /// in the `netmap_slot`, so that it can be given back to
    /// netmap to receive more packets.
    free_ring: CircularQueue<u32>,
}
// fields rx and tx removed because redundant with
// base.rx_ring.is_some() and base.tx_ring.is_some()


impl NethunsSocketNetmap {
    /// Create a new `NethunsSocketNetmap` object.
    pub(super) fn new(
        base: NethunsSocketBase,
        p: NmPortDescriptor,
        some_ring: NetmapRing,
        free_ring: CircularQueue<u32>,
    ) -> Self {
        Self {
            base,
            p,
            some_ring,
            free_ring,
        }
    }
}


impl NethunsSocketInnerTrait for NethunsSocketNetmap {
    fn recv(&mut self) -> Result<RecvPacket, NethunsRecvError> {
        // Check if the ring has been binded to a queue and if it's in RX mode
        let rx_ring = match &mut self.base.rx_ring {
            Some(r) => r,
            None => return Err(NethunsRecvError::NotRx),
        };
        
        // Get the first slot available to userspace (head of RX ring) and check if it's in use
        let head_idx = rx_ring.head();
        if rx_ring.get_slot(head_idx).status.load(Ordering::Acquire)
            != RingSlotStatus::Free
        {
            return Err(NethunsRecvError::InUse);
        }
        
        // If no slots are available, try again after
        // taking some available "extra buffers" as "free slots"
        if self.free_ring.is_empty() {
            nethuns_ring_free_slots!(self, rx_ring, nethuns_blocks_free);
            
            if self.free_ring.is_empty() {
                return Err(NethunsRecvError::NoPacketsAvailable);
            }
            
            // Check some invariant during debugging
            debug_assert_ne!(
                self.free_ring.clone_get(self.free_ring.head()),
                0
            );
            debug_assert_ne!(
                self.free_ring.clone_get(self.free_ring.tail() - 1),
                0
            );
        }
        
        // Find the first non-empty netmap ring.
        let mut netmap_ring = match non_empty_rx_ring(&mut self.p) {
            Ok(r) => r,
            Err(_) => {
                // All netmap rings are empty.
                // Try again after synchronizing the rx rings
                // of the socket.
                // If it still fails, return an error
                // (no packets available at the moment).
                unsafe { libc::ioctl(self.p.fd, NIOCRXSYNC) };
                non_empty_rx_ring(&mut self.p)?
            }
        };
        
        // Get a newly received packet from the `cur` netmap ring slot
        // (first available slot not owned by userspace).
        let i = netmap_ring.cur;
        let mut cur_netmap_slot = netmap_ring
            .get_slot(i as _)
            .map_err(NethunsRecvError::Error)?;
        let idx = cur_netmap_slot.buf_idx;
        let pkt = unsafe { netmap_buf_pkt!(netmap_ring, idx) };
        
        // Update the packet header metadata of the nethuns ring abstraction
        // against the actual netmap packet.
        {
            let slot = rx_ring.get_slot_mut(head_idx);
            slot.pkthdr.ts = netmap_ring.ts;
            slot.pkthdr.caplen = cur_netmap_slot.len as _;
            slot.pkthdr.len = cur_netmap_slot.len as _;
            slot.pkthdr.buf_idx = idx;
        }
        
        // Assign a new buffer to the netmap `cur` slot and set the relative flag
        cur_netmap_slot.buf_idx = self.free_ring.clone_pop_unchecked();
        cur_netmap_slot.flags |= NS_BUF_CHANGED as u16;
        
        // Move `cur` and `head` indexes ahead of one position
        netmap_ring.cur = unsafe { netmap_ring.nm_ring_next(i) };
        netmap_ring.head = unsafe { netmap_ring.nm_ring_next(i) };
        
        // Filter the packet
        if match &self.base.filter {
            None => false,
            Some(filter) => {
                // Call the filter closure
                !filter(&rx_ring.get_slot(head_idx).pkthdr, pkt)
            }
        } {
            nethuns_ring_free_slots!(self, rx_ring, nethuns_blocks_free);
            return Err(NethunsRecvError::PacketFiltered);
        }
        
        {
            let slot = rx_ring.get_slot_mut(head_idx);
            slot.pkthdr.caplen =
                cmp::min(self.base.opt.packetsize, slot.pkthdr.caplen);
            slot.status.store(RingSlotStatus::InUse, Ordering::Release);
        }
        
        rx_ring.rings_mut().advance_head();
        
        let recv_packet = {
            // IMPORTANT!! slot MUST be an **immutable** reference,
            // otherwise the Rust memory model rules will be broken.
            let slot = rx_ring.get_slot(head_idx);
            
            RecvPacket::new(
                rx_ring.head() as _,
                &slot.pkthdr,
                pkt,
                &slot.status,
            )
        };
        
        Ok(recv_packet)
    }
    
    
    fn send(&mut self, packet: &[u8]) -> Result<(), NethunsSendError> {
        let tx_ring = match &mut self.base.tx_ring {
            Some(r) => r,
            None => return Err(NethunsSendError::NotTx),
        };
        
        let slot = tx_ring.get_slot(tx_ring.tail());
        if slot.status.load(Ordering::Relaxed) != RingSlotStatus::Free {
            return Err(NethunsSendError::InUse);
        }
        
        let dst = unsafe {
            nethuns_get_buf_addr_netmap!(
                &self.some_ring,
                tx_ring,
                tx_ring.tail()
            )
        };
        
        if packet.len() > dst.len() as _ {
            return Err(NethunsSendError::InvalidPacketSize(
                dst.len(),
                packet.len(),
            ));
        }
        
        unsafe {
            // [SAFETY] we checked that `packet.len()` <= `dst.len()`
            nm_pkt_copy(
                packet.as_ptr() as _,
                dst.as_mut_ptr() as _,
                packet.len() as _,
            )
        };
        tx_ring.nethuns_send_slot(tx_ring.tail(), packet.len());
        tx_ring.rings_mut().advance_tail();
        
        Ok(())
    }
    
    
    fn flush(&mut self) -> Result<(), NethunsFlushError> {
        let tx_ring = match &mut self.base.tx_ring {
            Some(r) => r,
            None => return Err(NethunsFlushError::NotTx),
        };
        
        let mut prev_tails: Box<[u32]> =
            vec![0; (self.p.last_tx_ring - self.p.first_tx_ring + 1) as _]
                .into_boxed_slice();
        
        let mut head = tx_ring.head();
        
        // Try to push packets marked for transmission
        for i in (self.p.first_tx_ring as _)..=(self.p.last_tx_ring as _) {
            let mut ring = NetmapRing::new(
                NonNull::new(
                    unsafe { netmap_txring(self.p.nifp, i) }
                )
                .ok_or(
                    NethunsFlushError::FrameworkError(
                        "failed to initialize some_ring: netmap_txring returned null".to_owned()
                    )
                )?
            );
            prev_tails[i - self.p.first_tx_ring as usize] = ring.tail;
            
            loop {
                let slot = tx_ring.get_slot_mut(head);
                
                if ring.nm_ring_empty()
                    || slot.status.load(Ordering::Acquire)
                        != RingSlotStatus::InUse
                {
                    break;
                }
                
                // swap buf indexes between the nethuns and netmap slots, mark
                // the nethuns slot as in-flight
                slot.status
                    .store(RingSlotStatus::InFlight, Ordering::Relaxed);
                let mut netmap_slot = ring
                    .get_slot(ring.head as _)
                    .map_err(NethunsFlushError::FrameworkError)?;
                mem::swap(&mut netmap_slot.buf_idx, &mut slot.pkthdr.buf_idx);
                netmap_slot.len =
                    u16::try_from(slot.len).unwrap_or_else(|_| {
                        panic!(
                            "integer overflow: couldn't convert {} to u16",
                            slot.len
                        )
                    });
                netmap_slot.flags = NS_BUF_CHANGED as _;
                // remember the nethuns slot in the netmap slot ptr field
                netmap_slot.ptr = &*slot as *const NethunsRingSlot as _;
                
                ring.cur = unsafe { ring.nm_ring_next(ring.head) };
                ring.head = ring.cur;
                head += 1;
                tx_ring.rings_mut().advance_head();
            }
        }
        
        if unsafe { libc::ioctl(self.p.fd, NIOCTXSYNC) < 0 } {
            return Err(NethunsFlushError::Error(format!(
                "ioctl({:?}, {:?}) failed with errno {}",
                self.p.fd,
                NIOCTXSYNC,
                errno::errno()
            )));
        }
        
        // cleanup completed transmissions: for each completed
        // netmap slot, mark the corresponding nethuns slot as
        // available (status <- Free)
        for i in (self.p.first_tx_ring as _)..=(self.p.last_tx_ring as _) {
            let ring = NetmapRing::new(
                NonNull::new(
                    unsafe { netmap_txring(self.p.nifp, i) }
                )
                .ok_or(
                    NethunsFlushError::FrameworkError(
                        "failed to initialize some_ring: netmap_txring returned null".to_owned()
                    )
                )?
            );
            
            let stop = unsafe { ring.nm_ring_next(ring.tail) };
            let mut scan = unsafe {
                ring.nm_ring_next(prev_tails[i - self.p.first_tx_ring as usize])
            };
            
            while scan != stop {
                let mut netmap_slot = ring
                    .get_slot(scan as _)
                    .map_err(NethunsFlushError::FrameworkError)?;
                let slot =
                    unsafe { &mut *(netmap_slot.ptr as *mut NethunsRingSlot) };
                mem::swap(&mut netmap_slot.buf_idx, &mut slot.pkthdr.buf_idx);
                slot.status.store(RingSlotStatus::Free, Ordering::Release);
                
                scan = unsafe { ring.nm_ring_next(scan) };
            }
        }
        
        Ok(())
    }
    
    
    #[inline(always)]
    fn send_slot(
        &mut self,
        id: usize,
        len: usize,
    ) -> Result<(), NethunsSendError> {
        let tx_ring = match &mut self.base.tx_ring {
            Some(r) => r,
            None => return Err(NethunsSendError::NotTx),
        };
        if tx_ring.nethuns_send_slot(id, len) {
            Ok(())
        } else {
            Err(NethunsSendError::InUse)
        }
    }
    
    
    #[inline(always)]
    fn base(&self) -> &NethunsSocketBase {
        &self.base
    }
    
    #[inline(always)]
    fn base_mut(&mut self) -> &mut NethunsSocketBase {
        &mut self.base
    }
    
    
    #[inline(always)]
    fn fd(&self) -> std::os::raw::c_int {
        self.p.fd
    }
    
    
    #[inline(always)]
    fn get_packet_buffer_ref(&self, pktid: usize) -> Option<&mut [u8]> {
        self.base.tx_ring.as_ref().map(|tx_ring| unsafe {
            nethuns_get_buf_addr_netmap!(&self.some_ring, tx_ring, pktid)
        })
    }
    
    
    /// NOT IMPLEMENTED IN NETMAP
    #[inline(always)]
    fn fanout(&mut self, _: libc::c_int, _: &CStr) -> bool {
        false
    }
    
    /// NOT IMPLEMENTED IN NETMAP
    #[inline(always)]
    fn dump_rings(&mut self) {}
    
    #[inline(always)]
    fn stats(&self) -> Option<NethunsStat> {
        Some(NethunsStat::default())
    }
}


impl Drop for NethunsSocketNetmap {
    fn drop(&mut self) {
        // Clear promisc mode of interface if previously set
        if self.base.opt.promisc {
            if let Err(e) = nethuns_clear_if_promisc(&self.base.devname) {
                eprintln!("[NethunsSocketNetmap::Drop] couldn't clear promisc mode: {e}");
            }
        }
        
        if let Some(ring) = &self.base.tx_ring {
            for i in 0..ring.size() {
                let idx = ring.get_slot(i).pkthdr.buf_idx;
                let next = unsafe {
                    netmap_buf(&self.some_ring, idx as _) as *mut u32
                };
                debug_assert!(!next.is_null());
                unsafe {
                    *next = (*self.p.nifp).ni_bufs_head;
                    (*self.p.nifp).ni_bufs_head = idx;
                };
            }
        }
        
        while !self.free_ring.is_empty() {
            let idx = self.free_ring.clone_pop_unchecked();
            let next =
                unsafe { netmap_buf(&self.some_ring, idx as _) as *mut u32 };
            debug_assert!(!next.is_null());
            unsafe {
                *next = (*self.p.nifp).ni_bufs_head;
                (*self.p.nifp).ni_bufs_head = idx;
            };
        }
    }
}
