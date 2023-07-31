use std::ffi::CString;
use std::{ptr, thread, time};

use c_netmap_wrapper::bindings::netmap_ring;
use c_netmap_wrapper::macros::{netmap_buf, netmap_rxring};
use c_netmap_wrapper::nmport::NmPortDescriptor;
use c_netmap_wrapper::ring::NetmapRing;

use crate::api::nethuns_dev_queue_name;
use crate::nethuns::__nethuns_set_if_promisc;
use crate::sockets::base::NethunsSocketBase;
use crate::sockets::errors::{NethunsBindError, NethunsOpenError};
use crate::sockets::ring::{nethuns_lpow2, NethunsRing};
use crate::sockets::NethunsSocket;
use crate::types::{NethunsQueue, NethunsSocketMode, NethunsSocketOptions};


#[derive(Debug)]
pub struct NethunsSocketNetmap {
    base: NethunsSocketBase,
    p: Option<NmPortDescriptor>,
    some_ring: Option<NetmapRing>,
    free_ring: Vec<u32>,
    free_mask: u64,
    free_head: u64,
    free_tail: u64,
    tx: bool,
    rx: bool,
}


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
            base.rx_ring = Some(NethunsRing::new(
                (opt.numblocks * opt.numpackets) as usize,
                opt.packetsize as usize,
            ));
        }
        
        if tx {
            base.tx_ring = Some(NethunsRing::new(
                (opt.numblocks * opt.numpackets) as usize,
                opt.packetsize as usize,
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
            tx,
            rx,
        }))
    }
    
    
    /// TODO better error type
    fn bind(
        &mut self,
        dev: &str,
        queue: NethunsQueue,
    ) -> Result<(), NethunsBindError> {
        let flags = if !self.tx {
            "/R".to_owned()
        } else if !self.rx {
            "/T".to_owned()
        } else {
            panic!("NethunsSocket should be either rx or tx");
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
        .or_else(|e| {
            Err(NethunsBindError::IllegalArgument(format!(
                "Unable to build the device name as CString: {e}"
            )))
        })?;
        
        // Prepare the port descriptor
        let mut nm_port_d = NmPortDescriptor::prepare(nm_dev).or_else(|e| {
            Err(NethunsBindError::FrameworkError(format!(
                "could not open dev {} ({e})",
                nethuns_dev_queue_name(Some(dev), queue)
            )))
        })?;
        
        // Get device name
        let c_dev = CString::new(dev.to_owned()).or_else(|e| {
            Err(NethunsBindError::IllegalArgument(format!(
                "Unable to convert `dev` ({dev}) to CString: {e}"
            )))
        })?;
        
        // Configure NethunsSocketBase structure
        self.base.queue = queue;
        self.base.ifindex =
            unsafe { libc::if_nametoindex(c_dev.as_ptr()) } as i32;
        
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
        let extra_bufs =
            (self.tx as u32) * rx_ring_size + (self.rx as u32) * tx_ring_size;
        nm_port_d.d.reg.nr_extra_bufs = extra_bufs;
        
        // open initialized port descriptor
        nm_port_d.open_desc().or_else(|e| {
            Err(NethunsBindError::FrameworkError(format!(
                "NmPortDescriptor.open_desc(): couldn't open dev {} ({})",
                nethuns_dev_queue_name(Some(&dev), queue),
                e
            )))
        })?;
        
        if nm_port_d.d.reg.nr_extra_bufs != extra_bufs {
            return Err(NethunsBindError::FrameworkError(format!(
                "dev {}: cannot obtain {} extra bufs (got {})",
                nethuns_dev_queue_name(Some(dev), queue),
                extra_bufs,
                nm_port_d.d.reg.nr_extra_bufs
            )));
        }
        
        let some_ring = NetmapRing::from(netmap_rxring(
            nm_port_d.d.nifp,
            if self.rx {
                nm_port_d.d.first_rx_ring as usize
            } else {
                nm_port_d.d.first_tx_ring as usize
            },
        ));
        
        let extra_bufs = nethuns_lpow2(nm_port_d.d.reg.nr_extra_bufs as usize);
        self.free_ring = vec![0; extra_bufs];
        self.free_mask = (extra_bufs - 1) as u64;
        
        let mut scan = unsafe {
            assert!(!nm_port_d.d.nifp.is_null());
            (*nm_port_d.d.nifp).ni_bufs_head
        };
        
        if let Some(tx_ring) = &mut self.base.tx_ring {
            for i in 0..tx_ring.size {
                let slot = tx_ring.get_slot(i);
                (*slot).pkthdr.buf_idx = scan;
                scan = unsafe {
                    let ptr = netmap_buf(&some_ring.r, i) as *const u32;
                    assert!(!ptr.is_null());
                    *ptr
                }
            }
        }
        
        if self.rx {
            while scan != 0 {
                self.free_ring[(self.free_tail & self.free_mask) as usize] =
                    scan;
                self.free_tail += 1;
                scan = unsafe {
                    let ptr =
                        netmap_buf(&some_ring.r, scan as usize) as *const u32;
                    assert!(!ptr.is_null());
                    *ptr
                };
            }
        }
        unsafe {
            (*nm_port_d.d.nifp).ni_bufs_head = 0;
        }
        
        self.base.devname = c_dev;
        
        if self.base.opt.promisc {
            __nethuns_set_if_promisc(self, &self.base.devname).or_else(|e| {
                Err(NethunsBindError::NethunsError(format!(
                    "couldn't set promisc mode: {e}"
                )))
            })?
        }
        
        // Move generated fields to NethunsSocket struct
        self.p = Some(nm_port_d);
        self.some_ring = Some(some_ring);
        
        thread::sleep(time::Duration::from_secs(2));
        Ok(())
    }
}


impl Drop for NethunsSocketNetmap {
    fn drop(&mut self) {
        todo!();
        
        // if self.base.opt.promisc {
        //     //__nethuns_clear_if_promisc(s, b->devname);
        //     todo!();
        // }
        
        // if let None = &self.p {
        //     return; // TODO
        // }
        
        // let nifp = &self.p.as_ref().unwrap().d.nifp;
        
        // if self.tx {
        //     if let Some(ring) = &self.base.tx_ring {
        //         for i in 0..ring.size {
        //             let slot = ring.get_slot(i);
        //             let idx = slot.pkthdr.buf_idx;
        //             let next =
        //                 netmap_buf(&self.some_ring, idx as usize) as *const
        // u32;             unsafe {
        //                 // *next = (*nifp).
        //             }
        //         }
        //     }
        // }
    }
}
