use std::ffi::CString;
use std::rc::Rc;
use std::sync::atomic;
use std::{thread, time};

use c_netmap_wrapper::bindings::NS_BUF_CHANGED;
use c_netmap_wrapper::constants::NIOCRXSYNC;
use c_netmap_wrapper::macros::{netmap_buf, netmap_rxring};
use c_netmap_wrapper::netmap_buf_pkt;
use c_netmap_wrapper::nmport::NmPortDescriptor;
use c_netmap_wrapper::ring::NetmapRing;

use crate::api::nethuns_dev_queue_name;
use crate::misc::macros::min;
use crate::nethuns::{__nethuns_clear_if_promisc, __nethuns_set_if_promisc};
use crate::sockets::base::{NethunsSocketBase, RecvPacket};
use crate::sockets::errors::{
    NethunsBindError, NethunsOpenError, NethunsRecvError,
};
use crate::sockets::netmap::ring::non_empty_rx_ring;
use crate::sockets::ring::{
    nethuns_lpow2, nethuns_ring_free_slots, NethunsRing,
};
use crate::sockets::NethunsSocket;
use crate::types::{NethunsQueue, NethunsSocketMode, NethunsSocketOptions};


#[derive(Debug)]
pub struct NethunsSocketNetmap {
    base: NethunsSocketBase,
    p: Option<NmPortDescriptor>,
    some_ring: Option<NetmapRing>, // ? chiedere a Lettieri a che cosa serve some_ring
    free_ring: Vec<u32>,
    free_mask: u64,
    free_head: u64,
    free_tail: u64,
}
// fields rx and tx removed because redundant with base.rx_ring.is_some() and
// base.tx_ring.is_some()


impl NethunsSocket for NethunsSocketNetmap {
    /// Create a new NethunsSocket
    fn try_new(
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
            base.rx_ring = Some(
                NethunsRing::try_new(
                    (opt.numblocks * opt.numpackets) as usize,
                    opt.packetsize as usize,
                )
                .map_err(NethunsOpenError::AllocationError)?,
            );
        }
        
        if tx {
            base.tx_ring = Some(
                NethunsRing::try_new(
                    (opt.numblocks * opt.numpackets) as usize,
                    opt.packetsize as usize,
                )
                .map_err(NethunsOpenError::AllocationError)?,
            );
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
    
    
    /// TODO better error type
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
            unsafe { libc::if_nametoindex(c_dev.as_ptr()) } as libc::c_int;
        
        // Configure the Netmap port descriptor
        // with the number of required extra buffers
        let rx_ring_size = match &self.base.rx_ring {
            Some(r) => r.size,
            None => 0,
        } as u32;
        let tx_ring_size = match &self.base.tx_ring {
            Some(r) => r.size,
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
        let some_ring = NetmapRing::try_new(unsafe {
            assert!(!nm_port_d.nifp.is_null());
            netmap_rxring(
                nm_port_d.nifp,
                if self.rx() {
                    nm_port_d.first_rx_ring as usize
                } else {
                    nm_port_d.first_tx_ring as usize
                },
            )
        })
        .map_err(|e| {
            NethunsBindError::NethunsError(format!(
                "failed to initialize some_ring: {e}"
            ))
        })?;
        
        // Initialize free_ring and free_mask
        let extra_bufs = nethuns_lpow2(nm_port_d.reg.nr_extra_bufs as usize);
        self.free_ring = vec![0; extra_bufs];
        self.free_mask = (extra_bufs - 1) as u64;
        
        
        // Retrieve the ring slots generated by the kernel
        let mut scan = unsafe { (*nm_port_d.nifp).ni_bufs_head };
        // Case 1: TX
        if let Some(tx_ring) = &mut self.base.tx_ring {
            for i in 0..tx_ring.size {
                tx_ring.get_slot(i).borrow_mut().pkthdr.buf_idx = scan;
                scan = unsafe {
                    let ptr =
                        netmap_buf(&some_ring, scan as usize) as *const u32;
                    *ptr
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
                    let ptr =
                        netmap_buf(&some_ring, scan as usize) as *const u32;
                    assert!(!ptr.is_null());
                    *ptr
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
                NethunsBindError::NethunsError(format!(
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
    
    
    ///
    fn recv(&mut self) -> Result<RecvPacket, NethunsRecvError> {
        // Check if the ring has been binded to a queue and it's in RX mode
        let nmport_d: &mut NmPortDescriptor = match &mut self.p {
            Some(p) => p,
            None => return Err(NethunsRecvError::NonBinded),
        };
        let rx_ring = match &mut self.base.rx_ring {
            Some(r) => r,
            None => return Err(NethunsRecvError::NotRx),
        };
        
        let caplen = self.base.opt.packetsize;
        
        
        let rc_slot = rx_ring.get_slot(rx_ring.head as usize);
        let mut slot = rc_slot.borrow_mut();
        if slot.inuse.load(atomic::Ordering::Acquire) {
            return Err(NethunsRecvError::InUse);
        }
        if self.free_head == self.free_tail {
            // $$ unlikely
            nethuns_ring_free_slots!(self, rx_ring, slot, nethuns_blocks_free);
            if self.free_head == self.free_tail {
                return Err(NethunsRecvError::NethunsError("".to_owned())); // FIXME better error
            }
        }
        
        let mut netmap_ring = match non_empty_rx_ring(nmport_d) {
            Ok(r) => r,
            Err(_) => {
                unsafe { libc::ioctl(nmport_d.fd, NIOCRXSYNC) };
                non_empty_rx_ring(nmport_d)?
            }
        };
        
        let i = netmap_ring.cur;
        let mut cur_netmap_slot = netmap_ring
            .get_slot(i as usize)
            .map_err(NethunsRecvError::NethunsError)?;
        let idx = cur_netmap_slot.buf_idx;
        let pkt = netmap_buf_pkt!(netmap_ring, idx);
        
        slot.pkthdr.ts = netmap_ring.ts;
        slot.pkthdr.caplen = cur_netmap_slot.len as u32;
        slot.pkthdr.len = cur_netmap_slot.len as u32;
        
        cur_netmap_slot.buf_idx =
            self.free_ring[(self.free_head & self.free_mask) as usize];
        self.free_head += 1;
        cur_netmap_slot.flags |= NS_BUF_CHANGED as u16;
        
        netmap_ring.cur = netmap_ring.nm_ring_next(i);
        netmap_ring.head = netmap_ring.nm_ring_next(i);
        
        if match &self.base.filter {
            None => true,
            Some(filter) => filter(&slot.pkthdr, pkt) != 0,
        } {
            slot.pkthdr.caplen = min!(caplen, slot.pkthdr.caplen);
            
            slot.inuse.store(true, atomic::Ordering::Release);
            
            rx_ring.head += 1;
            
            return Ok(RecvPacket::try_new(
                rx_ring.head,
                Box::new(slot.pkthdr),
                pkt,
                Rc::downgrade(&rc_slot),
            )?);
        }
        
        nethuns_ring_free_slots!(self, rx_ring, slot, nethuns_blocks_free);
        Err(NethunsRecvError::PacketFiltered)
    }
    
    
    ///
    #[inline(always)]
    fn get_socket_base(&mut self) -> &mut NethunsSocketBase {
        &mut self.base
    }
}


impl Drop for NethunsSocketNetmap {
    fn drop(&mut self) {
        // FIXME check if return here is correct/safe
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
            for i in 0..ring.size {
                let idx = ring.get_slot(i).borrow().pkthdr.buf_idx;
                let next = netmap_buf(some_ring, idx as usize) as *mut u32;
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
            let next = netmap_buf(some_ring, idx as usize) as *mut u32;
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
    #[inline(always)]
    fn tx(&self) -> bool {
        self.base.tx_ring.is_some()
    }
    
    #[inline(always)]
    fn rx(&self) -> bool {
        self.base.rx_ring.is_some()
    }
}


macro_rules! nethuns_blocks_free {
    ($s: expr, $slot: expr) => {
        $s.free_ring[($s.free_tail & $s.free_mask) as usize] =
            $slot.pkthdr.buf_idx;
        $s.free_tail += 1;
    };
}
pub(self) use nethuns_blocks_free;
