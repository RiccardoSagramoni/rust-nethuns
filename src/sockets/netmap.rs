use std::ptr;

use c_netmap_wrapper::{netmap_ring, nmport_d};

use crate::types::{NethunsSocketMode, NethunsSocketOptions};

use super::base::NethunsSocketBase;
use super::errors::NethunsOpenError;
use super::ring::NethunsRing;


#[derive(Debug)]
pub struct NethunsSocket {
    base: NethunsSocketBase,
    p: nmport_d,            // TODO destructor
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
            p: nmport_d::default(),
            some_ring: netmap_ring::default(),
            free_ring: ptr::null(),
            free_mask: 0,
            free_head: 0,
            free_tail: 0,
            tx,
            rx,
        })
    }
}


impl Drop for NethunsSocket {
    
    fn drop(&mut self) {
        if self.base.opt.promisc {
            //__nethuns_clear_if_promisc(s, b->devname);
            todo!();
        }
        
        let nifp = &self.p.nifp;
        
        if self.tx {
            if let Some(ring) = self.base.tx_ring {
                for i in 0..ring.size {
                    // TODO HERE
                }
            }
        }
    }
    
}
