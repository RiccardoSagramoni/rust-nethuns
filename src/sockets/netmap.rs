use std::error::Error;
use std::ptr;

use derivative::Derivative;
use derive_builder::Builder;

use crate::types::{NethunsSocketMode, NethunsSocketOptions};

use super::bindings::{netmap_ring, nmport_d};

use super::base::NethunsSocketBase;
use super::errors::NethunsOpenError;
use super::ring;


#[derive(Builder, Debug, Derivative)]
#[builder(pattern = "owned", default)]
#[derivative(Default)]
pub struct NethunsSocket {
    pub base: NethunsSocketBase,
    pub p: nmport_d, // TODO destructor
    pub some_ring: netmap_ring, // TODO destructor
    
    #[derivative(Default(value = "ptr::null()"))]
    pub free_ring: *const u32, // TODO check usage to wrap unsafe behavior
    
    pub free_mask: u64,
    pub free_head: u64,
    pub free_tail: u64,
    pub tx: bool,
    pub rx: bool,
}


///
pub fn nethuns_open(
    opt: &NethunsSocketOptions,
) -> Result<NethunsSocket, NethunsOpenError> {
    // TODO define a better error type
    
    let mut s = NethunsSocket::default();
    
    s.rx = opt.mode == NethunsSocketMode::RxTx
        || opt.mode == NethunsSocketMode::RxOnly;
    s.tx = opt.mode == NethunsSocketMode::RxTx
        || opt.mode == NethunsSocketMode::TxOnly;
    
    if !s.rx && !s.tx {
        return Err(NethunsOpenError::InvalidOptions(
            "please select at least one between rx and tx".to_owned(),
        ));
    }
    
    if s.rx {
        ring::nethuns_make_ring(
            (opt.numblocks * opt.numpackets) as usize,
            opt.packetsize as usize,
            &mut s.base.rx_ring,
        );
    }
    
    if s.tx {
        ring::nethuns_make_ring(
            (opt.numblocks * opt.numpackets) as usize,
            opt.packetsize as usize,
            &mut s.base.tx_ring,
        );
    }
    
    // set a single consumer by default
    s.base.opt = opt.clone(); // TODO clone or ref & ?
    
    Ok(s)
}
