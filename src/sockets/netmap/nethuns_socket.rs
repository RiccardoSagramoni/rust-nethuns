use std::ffi::CString;
use std::{thread, time};

use c_netmap_wrapper::macros::{netmap_buf, netmap_rxring};
use c_netmap_wrapper::nmport::NmPortDescriptor;
use c_netmap_wrapper::ring::NetmapRing;

use crate::api::nethuns_dev_queue_name;
use crate::nethuns::{__nethuns_clear_if_promisc, __nethuns_set_if_promisc};
use crate::sockets::base::NethunsSocketBase;
use crate::sockets::errors::{NethunsBindError, NethunsOpenError};
use crate::sockets::ring::{nethuns_lpow2, NethunsRing};
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
                let slot = tx_ring.get_slot(i);
                slot.pkthdr.buf_idx = scan;
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
    fn recv(&self) -> Result<(), String> { // FIXME return tuple with pkthds, payload and pkt id
        
        let caplen = self.base.opt.packetsize;
        
        let rx_ring = match &self.base.rx_ring {
            Some(r) => r,
            None => todo!(), // TODO error (socket not in send mode)
        };
        
        let slot = rx_ring.get_slot(rx_ring.head as usize);
        // slot.inuse.fetch_add(val, order);
        
        todo!()
    }
    
    
    ///
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
                let slot = ring.get_slot(i);
                let idx = slot.pkthdr.buf_idx;
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
    fn tx(&self) -> bool {
        self.base.tx_ring.is_some()
    }

    fn rx(&self) -> bool {
        self.base.rx_ring.is_some()
    }
}
