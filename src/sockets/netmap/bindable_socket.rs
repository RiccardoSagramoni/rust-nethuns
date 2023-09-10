use std::ffi::CString;
use std::ptr::NonNull;
use std::{thread, time};

use c_netmap_wrapper::macros::{netmap_buf, netmap_rxring};
use c_netmap_wrapper::nmport::NmPortDescriptor;
use c_netmap_wrapper::ring::NetmapRing;

use crate::misc::{nethuns_dev_queue_name, CircularCloneBuffer};
use crate::nethuns::__nethuns_set_if_promisc;
use crate::sockets::base::NethunsSocketBase;
use crate::sockets::errors::{NethunsBindError, NethunsOpenError};
use crate::sockets::ring::NethunsRing;
use crate::sockets::BindableNethunsSocket;
use crate::types::{NethunsQueue, NethunsSocketMode, NethunsSocketOptions};

use super::nethuns_socket::NethunsSocketNetmap;
use super::super::NethunsSocket;


#[derive(Debug)]
pub(crate) struct BindableNethunsSocketNetmap {
    base: NethunsSocketBase,
}



impl BindableNethunsSocketNetmap {
    /// Open a new Nethuns socket for the `netmap` framework.
    ///
    /// # Arguments
    /// * `opt`: The options for the socket.
    ///
    /// # Returns
    /// * `Ok(Box<dyn BindableNethunsSocket>)` - A new nethuns socket, in no error occurs.
    /// * `Err(NethunsOpenError::InvalidOptions)` - If at least one of the options holds a invalid value.
    /// * `Err(NethunsOpenError::Error)` - If an unexpected error occurs.
    pub fn open(
        opt: NethunsSocketOptions,
    ) -> Result<Box<dyn BindableNethunsSocket>, NethunsOpenError> {
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
        
        Ok(Box::new(Self { base }))
    }
}



impl BindableNethunsSocket for BindableNethunsSocketNetmap {
    fn bind(
        mut self: Box<Self>,
        dev: &str,
        queue: NethunsQueue,
    ) -> Result<Box<dyn NethunsSocket>, (NethunsBindError, Box<dyn BindableNethunsSocket>)> {
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
        let nm_dev = match CString::new(match queue {
            NethunsQueue::Some(idx) => {
                format!("{prefix}{dev}-{idx}{flags}")
            }
            NethunsQueue::Any => {
                format!("{prefix}{dev}{flags}")
            }
        }) {
            Ok(nm_dev) => nm_dev,
            Err(e) => {
                return Err((
                    NethunsBindError::IllegalArgument(format!(
                        "Unable to build the device name as CString: {e}"
                    )),
                    self,
                ))
            }
        };
        
        // Convert the device name to a C string
        let c_dev = match CString::new(dev.to_owned()) {
            Ok(c_dev) => c_dev,
            Err(e) => {
                return Err((
                    NethunsBindError::IllegalArgument(format!(
                        "Unable to convert `dev` ({dev}) to CString: {e}"
                    )),
                    self,
                ))
            }
        };
        
        
        // Initialize a new netmap port descriptor
        let mut nm_port_d = match NmPortDescriptor::prepare(nm_dev) {
            Ok(nm_port_d) => nm_port_d,
            Err(e) => {
                return Err((
                    NethunsBindError::FrameworkError(format!(
                        "could not open dev {} ({e})",
                        nethuns_dev_queue_name(Some(dev), queue)
                    )),
                    self,
                ))
            }
        };
        
        
        // Configure the Netmap port descriptor
        // with the number of required extra buffers
        let rx_ring_size =
            self.base.rx_ring.as_ref().map(|r| r.size()).unwrap_or(0) as u32;
        let tx_ring_size =
            self.base.tx_ring.as_ref().map(|r| r.size()).unwrap_or(0) as u32;
        let extra_bufs = (if self.tx() { rx_ring_size } else { 0_u32 })
            + (if self.rx() { tx_ring_size } else { 0_u32 });
        nm_port_d.reg.nr_extra_bufs = extra_bufs;
        
        // Open the initialized netmap port descriptor
        if let Err(e) = nm_port_d.open_desc() {
            return Err((
                NethunsBindError::FrameworkError(format!(
                    "NmPortDescriptor.open_desc(): couldn't open dev {} ({})",
                    nethuns_dev_queue_name(Some(dev), queue),
                    e
                )),
                self,
            ));
        }
        
        // Check if the number of extra buffers is correct
        if nm_port_d.reg.nr_extra_bufs != extra_bufs {
            return Err((
                NethunsBindError::FrameworkError(format!(
                    "dev {}: cannot obtain {} extra bufs (got {})",
                    nethuns_dev_queue_name(Some(dev), queue),
                    extra_bufs,
                    nm_port_d.reg.nr_extra_bufs
                )),
                self,
            ));
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
            match NonNull::new(ptr) {
                Some(ptr) => ptr,
                None => {
                    return Err((
                        NethunsBindError::FrameworkError(
                            "failed to initialize some_ring: netmap_rxring returned null"
                                .to_owned()
                        ), 
                        self
                    ))
                }
            }
        });
        
        
        // TODO comment
        let mut free_ring =
            CircularCloneBuffer::new(nm_port_d.reg.nr_extra_bufs as _, &|| 0);
        
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
                free_ring.push_unchecked(scan);
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
        
        
        if self.base.opt.promisc {
            // Set the interface in promisc mode
            if let Err(e) = __nethuns_set_if_promisc(&c_dev) {
                return Err((
                    NethunsBindError::Error(format!(
                        "couldn't set promisc mode: {e}"
                    )),
                    self,
                ));
            }
        }
        
        
        // Configure the base socket descriptor
        self.base.devname = c_dev;
        self.base.queue = queue;
        self.base.ifindex =
            unsafe { libc::if_nametoindex(self.base.devname.as_ptr()) } as _;
        
        // Build the socket struct and return it
        let socket = NethunsSocketNetmap::new(self.base, nm_port_d, some_ring, free_ring);
        
        thread::sleep(time::Duration::from_secs(2));
        Ok(Box::new(socket))
    }
    
    
    #[inline(always)]
    fn base(&self) -> &NethunsSocketBase {
        &self.base
    }
    
    
    #[inline(always)]
    fn base_mut(&mut self) -> &mut NethunsSocketBase {
        &mut self.base
    }
}
