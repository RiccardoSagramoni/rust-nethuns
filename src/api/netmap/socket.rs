use std::ffi::{CStr, CString};
use std::ptr;

use c_netmap_wrapper::bindings::netmap_ring;
use c_netmap_wrapper::macros::netmap_buf;
use c_netmap_wrapper::nmport::NmPortDescriptor;

use crate::api::errors::{NethunsBindError, NethunsOpenError};
use crate::api::nethuns_dev_queue_name;
use crate::sockets::base::NethunsSocketBase;
use crate::sockets::ring::NethunsRing;
use crate::types::{NethunsQueue, NethunsSocketMode, NethunsSocketOptions};


#[derive(Debug)]
pub struct NethunsSocket {
    base: NethunsSocketBase,
    p: Option<NmPortDescriptor>,
    some_ring: netmap_ring, // TODO destructor
    free_ring: *const u32,  // TODO check usage to wrap unsafe behavior
    free_mask: u64,
    free_head: u64,
    free_tail: u64,
    tx: bool,
    rx: bool,
}


impl NethunsSocket {
    /// Create a new NethunsSocket
    fn try_new(
        opt: NethunsSocketOptions,
    ) -> Result<NethunsSocket, NethunsOpenError> {
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
        
        Ok(NethunsSocket {
            base,
            p: None,
            some_ring: netmap_ring::default(),
            free_ring: ptr::null(),
            free_mask: 0,
            free_head: 0,
            free_tail: 0,
            tx,
            rx,
        })
    }
    
    
    /// TODO better error type + queue u32?
    pub fn bind(
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
                nethuns_dev_queue_name(
                    Some(&dev),
                    queue
                ),
                e
            )))
        })?;
        
        if nm_port_d.d.reg.nr_extra_bufs != extra_bufs {
            return Err(NethunsBindError::FrameworkError(format!(
                "dev {}: cannot obtain {} extra bufs (got {})",
                nethuns_dev_queue_name(Some(dev), queue),
                extra_bufs,
                nm_port_d.d.reg.nr_extra_bufs
            )))
        }
        
        // TODO
        
        // Move nm port to Nethuns socket
        self.p = Some(nm_port_d);
        todo!();
    }
}


impl Drop for NethunsSocket {
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
        //                 netmap_buf(&self.some_ring, idx as usize) as *const u32;
        //             unsafe {
        //                 // *next = (*nifp).
        //             }
        //         }
        //     }
        // }
    }
}
