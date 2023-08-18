use std::ffi::{CStr, CString};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic;
use std::{cmp, mem, slice, thread, time};

use c_netmap_wrapper::bindings::{nm_pkt_copy, NS_BUF_CHANGED};
use c_netmap_wrapper::constants::{NIOCRXSYNC, NIOCTXSYNC};
use c_netmap_wrapper::macros::{netmap_buf, netmap_rxring, netmap_txring};
use c_netmap_wrapper::netmap_buf_pkt;
use c_netmap_wrapper::nmport::NmPortDescriptor;
use c_netmap_wrapper::ring::NetmapRing;

use crate::misc::{nethuns_dev_queue_name, nethuns_lpow2};
use crate::nethuns::{__nethuns_clear_if_promisc, __nethuns_set_if_promisc};
use crate::sockets::base::{NethunsSocketBase, RecvPacket};
use crate::sockets::errors::{
    NethunsBindError, NethunsFlushError, NethunsOpenError, NethunsRecvError,
    NethunsSendError,
};
use crate::sockets::netmap::utility::{nethuns_blocks_free, non_empty_rx_ring};
use crate::sockets::ring::{
    nethuns_ring_free_slots, NethunsRing, NethunsRingSlot,
};
use crate::sockets::NethunsSocket;
use crate::types::{
    NethunsQueue, NethunsSocketMode, NethunsSocketOptions, NethunsStat,
};

use super::utility::nethuns_get_buf_addr_netmap;


#[derive(Debug)]
pub(crate) struct NethunsSocketNetmap {
    base: NethunsSocketBase,
    p: Option<NmPortDescriptor>,
    some_ring: Option<NetmapRing>, // ?? a cosa serve?
    free_ring: Vec<u32>,
    free_mask: u64,
    free_head: u64,
    free_tail: u64,
}
// fields rx and tx removed because redundant with base.rx_ring.is_some() and
// base.tx_ring.is_some()


impl NethunsSocket for NethunsSocketNetmap {
    /// Create a new NethunsSocket
    fn open(
        opt: NethunsSocketOptions,
    ) -> Result<Box<dyn NethunsSocket>, NethunsOpenError> {
        let rx = opt.mode == NethunsSocketMode::RxTx
            || opt.mode == NethunsSocketMode::RxOnly;
        let tx = opt.mode == NethunsSocketMode::RxTx
            || opt.mode == NethunsSocketMode::TxOnly;
        
        if !rx && !tx {
            return Err(NethunsOpenError::InvalidOptions(
                "please select at least one between rx and tx".to_owned(),
            ));
        }
        
        let mut base = NethunsSocketBase::default();
        
        if rx {
            base.rx_ring = Some(NethunsRing::new(
                (opt.numblocks * opt.numpackets) as _,
                opt.packetsize as _,
            ));
        }
        
        if tx {
            base.tx_ring = Some(NethunsRing::new(
                (opt.numblocks * opt.numpackets) as _,
                opt.packetsize as _,
            ));
        }
        
        // set a single consumer by default
        base.opt = opt;
        
        Ok(Box::new(Self {
            base,
            p: None,
            some_ring: None,
            free_ring: Vec::new(),
            free_mask: 0,
            free_head: 0,
            free_tail: 0,
        }))
    }
    
    
    fn bind(
        &mut self,
        dev: &str,
        queue: NethunsQueue,
    ) -> Result<(), NethunsBindError> {
        // Prepare flag and prefix for device name
        let flags = if !self.tx() {
            "/R".to_owned()
        } else if !self.rx() {
            "/T".to_owned()
        } else {
            "".to_owned()
        };
        
        let prefix = if dev.starts_with("vale") {
            "".to_owned()
        } else {
            "netmap:".to_owned()
        };
        
        // Build the device name
        let nm_dev = CString::new(match queue {
            NethunsQueue::Some(idx) => {
                format!("{prefix}{dev}-{idx}{flags}")
            }
            NethunsQueue::Any => {
                format!("{prefix}{dev}{flags}")
            }
        })
        .map_err(|e| {
            NethunsBindError::IllegalArgument(format!(
                "Unable to build the device name as CString: {e}"
            ))
        })?;
        
        // Initialize a new netmap port descriptor
        let mut nm_port_d = NmPortDescriptor::prepare(nm_dev).map_err(|e| {
            NethunsBindError::FrameworkError(format!(
                "could not open dev {} ({e})",
                nethuns_dev_queue_name(Some(dev), queue)
            ))
        })?;
        
        // Convert the device name to a C string
        let c_dev = CString::new(dev.to_owned()).map_err(|e| {
            NethunsBindError::IllegalArgument(format!(
                "Unable to convert `dev` ({dev}) to CString: {e}"
            ))
        })?;
        
        // Configure NethunsSocketBase structure
        self.base.queue = queue;
        self.base.ifindex =
            unsafe { libc::if_nametoindex(c_dev.as_ptr()) } as _;
        
        // Configure the Netmap port descriptor
        // with the number of required extra buffers
        let rx_ring_size = match &self.base.rx_ring {
            Some(r) => r.size(),
            None => 0,
        } as u32;
        let tx_ring_size = match &self.base.tx_ring {
            Some(r) => r.size(),
            None => 0,
        } as u32;
        let extra_bufs = (if self.tx() { rx_ring_size } else { 0_u32 })
            + (if self.rx() { tx_ring_size } else { 0_u32 });
        nm_port_d.reg.nr_extra_bufs = extra_bufs;
        
        // Open the initialized netmap port descriptor
        nm_port_d.open_desc().map_err(|e| {
            NethunsBindError::FrameworkError(format!(
                "NmPortDescriptor.open_desc(): couldn't open dev {} ({})",
                nethuns_dev_queue_name(Some(dev), queue),
                e
            ))
        })?;
        
        // Check if the number of extra buffers is correct
        if nm_port_d.reg.nr_extra_bufs != extra_bufs {
            return Err(NethunsBindError::FrameworkError(format!(
                "dev {}: cannot obtain {} extra bufs (got {})",
                nethuns_dev_queue_name(Some(dev), queue),
                extra_bufs,
                nm_port_d.reg.nr_extra_bufs
            )));
        }
        
        // Initialize some_ring
        let some_ring = NetmapRing::new({
            assert!(!nm_port_d.nifp.is_null());
            let ptr = unsafe {
                netmap_rxring(
                    nm_port_d.nifp,
                    if self.rx() {
                        nm_port_d.first_rx_ring as _
                    } else {
                        nm_port_d.first_tx_ring as _
                    },
                )
            };
            NonNull::new(ptr).ok_or(NethunsBindError::FrameworkError(
                "failed to initialize some_ring: netmap_rxring returned null"
                    .to_owned(),
            ))?
        });
        
        // Initialize free_ring and free_mask
        let extra_bufs = nethuns_lpow2(nm_port_d.reg.nr_extra_bufs as _);
        self.free_ring = vec![0; extra_bufs];
        self.free_mask = (extra_bufs - 1) as _;
        
        
        // Retrieve the ring slots generated by the kernel
        let mut scan = unsafe { (*nm_port_d.nifp).ni_bufs_head };
        // Case 1: TX
        if let Some(tx_ring) = &mut self.base.tx_ring {
            for i in 0..tx_ring.size() {
                tx_ring.get_slot(i).borrow_mut().pkthdr.buf_idx = scan;
                scan = unsafe {
                    let ptr = netmap_buf(&some_ring, scan as _) as *const u32;
                    ptr.read_unaligned()
                }
            }
        }
        // Case 2: RX
        if self.rx() {
            while scan != 0 {
                self.free_ring[(self.free_tail & self.free_mask) as usize] =
                    scan;
                self.free_tail += 1;
                scan = unsafe {
                    let ptr = netmap_buf(&some_ring, scan as _) as *const u32;
                    assert!(!ptr.is_null());
                    ptr.read_unaligned()
                };
            }
        }
        // Reset the index of the first of the extra buffers
        unsafe {
            (*nm_port_d.nifp).ni_bufs_head = 0;
        }
        
        // Register the device name into the socket descriptor
        self.base.devname = c_dev;
        
        if self.base.opt.promisc {
            // Set the interface in promisc mode
            __nethuns_set_if_promisc(&self.base.devname).map_err(|e| {
                NethunsBindError::Error(format!(
                    "couldn't set promisc mode: {e}"
                ))
            })?
        }
        
        // Move the generated fields into the NethunsSocketNetmap object
        self.p = Some(nm_port_d);
        self.some_ring = Some(some_ring);
        
        thread::sleep(time::Duration::from_secs(2));
        Ok(())
    }
    
    
    fn recv(&mut self) -> Result<RecvPacket, NethunsRecvError> {
        // Check if the ring has been binded to a queue and if it's in RX mode
        let nmport_d = match &mut self.p {
            Some(p) => p,
            None => return Err(NethunsRecvError::NonBinded),
        };
        let rx_ring = match &mut self.base.rx_ring {
            Some(r) => r,
            None => return Err(NethunsRecvError::NotRx),
        };
        
        // Get the first slot available to userspace (head of RX ring) and check if it's in use
        let rc_slot = rx_ring.get_slot(rx_ring.head as _);
        let slot = rc_slot.borrow();
        if slot.inuse.load(atomic::Ordering::Acquire) != 0 {
            return Err(NethunsRecvError::InUse);
        }
        mem::drop(slot);
        
        if self.free_head == self.free_tail {
            nethuns_ring_free_slots!(self, rx_ring, nethuns_blocks_free);
            
            if self.free_head == self.free_tail {
                return Err(NethunsRecvError::NoPacketsAvailable); // FIXME better error
            }
        }
        
        // Find the first non-empty netmap ring.
        let mut netmap_ring = match non_empty_rx_ring(nmport_d) {
            Ok(r) => r,
            Err(_) => {
                // All netmap rings are empty.
                // Try again after telling the hardware of consumed packets
                // and asking for newly available packets.
                // If it still fails, return an error
                // (no packets available at the moment).
                unsafe { libc::ioctl(nmport_d.fd, NIOCRXSYNC) };
                non_empty_rx_ring(nmport_d)?
            }
        };
        
        // Get a newly received packet from the `cur` netmap ring slot
        // (first available slot not owned by userspace).
        let i = netmap_ring.cur;
        let mut cur_netmap_slot = netmap_ring
            .get_slot(i as _)
            .map_err(NethunsRecvError::Error)?;
        let idx = cur_netmap_slot.buf_idx;
        let pkt = netmap_buf_pkt!(netmap_ring, idx);
        
        // Update the packet header metadata of the nethuns ring abstraction against the actual netmap packet.
        let mut slot = rc_slot.borrow_mut();
        slot.pkthdr.ts = netmap_ring.ts;
        slot.pkthdr.caplen = cur_netmap_slot.len as _;
        slot.pkthdr.len = cur_netmap_slot.len as _;
        
        // Assign a new buffer to the netmap `cur` slot and set the relative flag
        cur_netmap_slot.buf_idx =
            self.free_ring[(self.free_head & self.free_mask) as usize];
        self.free_head += 1;
        cur_netmap_slot.flags |= NS_BUF_CHANGED as u16;
        
        // Move `cur` and `head` indexes ahead of one position
        netmap_ring.cur = netmap_ring.nm_ring_next(i);
        netmap_ring.head = netmap_ring.nm_ring_next(i);
        
        // Filter the packet
        if match &self.base.filter {
            None => true,
            Some(filter) => filter(&slot.pkthdr, pkt) != 0,
        } {
            slot.pkthdr.caplen =
                cmp::min(self.base.opt.packetsize, slot.pkthdr.caplen);
            
            slot.inuse.store(1, atomic::Ordering::Release);
            
            rx_ring.head += 1;
            
            return Ok(RecvPacket::new(
                rx_ring.head,
                Box::new(slot.pkthdr),
                pkt,
                Rc::downgrade(&rc_slot),
            ));
        }
        
        nethuns_ring_free_slots!(self, rx_ring, nethuns_blocks_free);
        Err(NethunsRecvError::PacketFiltered)
    }
    
    
    fn send(&mut self, packet: &[u8]) -> Result<(), NethunsSendError> {
        let tx_ring = match &mut self.base.tx_ring {
            Some(r) => r,
            None => return Err(NethunsSendError::NotTx),
        };
        let some_ring = match &self.some_ring {
            Some(r) => r,
            None => return Err(NethunsSendError::NonBinded),
        };
        
        let rc_slot = tx_ring.get_slot(tx_ring.tail as _);
        if rc_slot.borrow().inuse.load(atomic::Ordering::Relaxed) != 0 {
            return Err(NethunsSendError::InUse);
        }
        
        let dst =
            nethuns_get_buf_addr_netmap!(some_ring, tx_ring, tx_ring.tail);
        unsafe {
            nm_pkt_copy(packet.as_ptr() as _, dst as _, packet.len() as _)
        };
        tx_ring.nethuns_send_slot(tx_ring.tail, packet.len());
        tx_ring.tail += 1;
        
        Ok(())
    }
    
    
    fn flush(&mut self) -> Result<(), NethunsFlushError> {
        let tx_ring = match &mut self.base.tx_ring {
            Some(r) => r,
            None => return Err(NethunsFlushError::NotTx),
        };
        let nmport_d = match &mut self.p {
            Some(p) => p,
            None => return Err(NethunsFlushError::NonBinded),
        };
        
        let mut prev_tails: Vec<u32> =
            vec![0; (nmport_d.last_tx_ring - nmport_d.last_rx_ring + 1) as _];
        
        let mut head = tx_ring.head;
        
        // Try to push packets marked for transmission
        for i in nmport_d.first_tx_ring as _..=nmport_d.last_tx_ring as _ {
            let mut ring = NetmapRing::new(
                NonNull::new(
                    unsafe { netmap_txring(nmport_d.nifp, i) }
                )
                .ok_or(
                    NethunsFlushError::FrameworkError(
                        "failed to initialize some_ring: netmap_txring returned null".to_owned()
                    )
                )?
            );
            prev_tails[i - nmport_d.first_tx_ring as usize] = ring.tail;
            
            loop {
                let rc_slot = tx_ring.get_slot(head as _);
                let mut slot = rc_slot.borrow_mut();
                
                if ring.nm_ring_empty()
                    || slot.inuse.load(atomic::Ordering::Acquire) != 1
                {
                    break;
                }
                
                // swap buf indexes between the nethuns and netmap slots, mark
                // the nethuns slot as in-flight (inuse <- 2)
                slot.inuse.store(2, atomic::Ordering::Relaxed);
                let mut nslot = ring
                    .get_slot(ring.head as _)
                    .map_err(NethunsFlushError::FrameworkError)?;
                mem::swap(&mut nslot.buf_idx, &mut slot.pkthdr.buf_idx);
                nslot.len = slot.len as _;
                nslot.flags = NS_BUF_CHANGED as _;
                // remember the nethuns slot in the netmap slot ptr field
                nslot.ptr = &*slot as *const NethunsRingSlot as _;
                
                ring.cur = ring.nm_ring_next(ring.head);
                ring.head = ring.cur;
                head += 1;
                tx_ring.head = head;
            }
        }
        
        if unsafe { libc::ioctl(nmport_d.fd, NIOCTXSYNC) < 0 } {
            return Err(NethunsFlushError::Error(format!(
                "ioctl({:?}, {:?}) failed with errno {}",
                nmport_d.fd,
                NIOCTXSYNC,
                errno::errno()
            )));
        }
        
        // cleanup completed transmissions: for each completed
        // netmap slot, mark the corresponding nethuns slot as
        // available (inuse <- 0)
        for i in nmport_d.first_tx_ring as _..=nmport_d.last_tx_ring as _ {
            let ring = NetmapRing::new(
                NonNull::new(
                    unsafe { netmap_txring(nmport_d.nifp, i) }
                )
                .ok_or(
                    NethunsFlushError::FrameworkError(
                        "failed to initialize some_ring: netmap_txring returned null".to_owned()
                    )
                )?
            );
            
            let stop = ring.nm_ring_next(ring.tail);
            let mut scan = ring
                .nm_ring_next(prev_tails[i - nmport_d.first_tx_ring as usize]);
            
            while scan != stop {
                let mut nslot = ring
                    .get_slot(scan as _)
                    .map_err(NethunsFlushError::FrameworkError)?;
                let slot = unsafe { &mut *(nslot.ptr as *mut NethunsRingSlot) };
                mem::swap(&mut nslot.buf_idx, &mut slot.pkthdr.buf_idx);
                slot.inuse.store(0, atomic::Ordering::Release);
                
                scan = ring.nm_ring_next(scan);
            }
        }
        
        Ok(())
    }
    
    
    #[inline(always)]
    fn send_slot(&self, id: u64, len: usize) -> Result<(), NethunsSendError> {
        let tx_ring = match &self.base.tx_ring {
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
    fn socket_base(&self) -> &NethunsSocketBase {
        &self.base
    }
    
    #[inline(always)]
    fn socket_base_mut(&mut self) -> &mut NethunsSocketBase {
        &mut self.base
    }
    
    
    #[inline(always)]
    fn get_packet_buffer_ref(&self, pktid: u64) -> Option<&mut [u8]> {
        let some_ring = match &self.some_ring {
            Some(r) => r,
            None => return None,
        };
        let tx_ring = match &self.base.tx_ring {
            Some(r) => r,
            None => return None,
        };
        Some(unsafe {
            slice::from_raw_parts_mut(
                nethuns_get_buf_addr_netmap!(some_ring, tx_ring, pktid),
                self.base.opt.packetsize as _,
            )
        })
    }
    
    
    fn fd(&self) -> Option<libc::c_int> {
        self.p.as_ref().map(|p| p.fd)
    }
    
    /// NOT IMPLEMENTED IN NETMAP
    fn fanout(&mut self, _: libc::c_int, _: &CStr) -> bool {
        false
    }
    
    /// NOT IMPLEMENTED IN NETMAP
    fn dump_rings(&mut self) {}
    
    fn stats(&self) -> Option<NethunsStat> {
        Some(NethunsStat::default())
    }
}


impl Drop for NethunsSocketNetmap {
    fn drop(&mut self) {
        let nmport_d = match &mut self.p {
            Some(p) => p,
            None => return,
        };
        let some_ring = match &mut self.some_ring {
            Some(r) => r,
            None => return,
        };
        
        // Clear promisc mode of interface if previously set
        if self.base.opt.promisc {
            if let Err(e) = __nethuns_clear_if_promisc(&self.base.devname) {
                eprintln!("[NethunsSocketNetmap::Drop] couldn't clear promisc mode: {e}");
            }
        }
        
        if let Some(ring) = &self.base.tx_ring {
            for i in 0..ring.size() {
                let idx = ring.get_slot(i).borrow().pkthdr.buf_idx;
                let next = netmap_buf(some_ring, idx as _) as *mut u32;
                assert!(!next.is_null());
                unsafe {
                    *next = (*nmport_d.nifp).ni_bufs_head;
                    (*nmport_d.nifp).ni_bufs_head = idx;
                };
            }
        }
        
        while self.free_head != self.free_tail {
            let idx =
                self.free_ring[(self.free_head & self.free_mask) as usize];
            let next = netmap_buf(some_ring, idx as _) as *mut u32;
            assert!(!next.is_null());
            unsafe {
                *next = (*nmport_d.nifp).ni_bufs_head;
                (*nmport_d.nifp).ni_bufs_head = idx;
            };
            
            self.free_head += 1;
        }
    }
}


impl NethunsSocketNetmap {
    /// Check if the socket is in TX mode
    #[inline(always)]
    fn tx(&self) -> bool {
        self.base.tx_ring.is_some()
    }
    
    /// Check if the socket is in RX mode
    #[inline(always)]
    fn rx(&self) -> bool {
        self.base.rx_ring.is_some()
    }
}
