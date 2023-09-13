use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::{cmp, mem};

use libc::TCA_DUMP_INVISIBLE;
use pcap_sys::pcap_file_header;

use crate::sockets::base::pcap::constants::{
    KUZNETZOV_TCPDUMP_MAGIC, NSEC_TCPDUMP_MAGIC, TCPDUMP_MAGIC,
};
use crate::sockets::base::{NethunsSocketBase, RecvPacket};
use crate::sockets::errors::{NethunsPcapOpenError, NethunsPcapReadError};
use crate::sockets::ring::NethunsRing;
use crate::types::NethunsSocketOptions;

use super::{NethunsSocketPcap, NethunsSocketPcapTrait};


// Define the type of the built-in pcap reader
pub type PcapReaderType = File;


impl NethunsSocketPcapTrait for NethunsSocketPcap {
    fn open(
        opt: NethunsSocketOptions,
        filename: &str,
        writing_mode: bool,
    ) -> Result<Self, NethunsPcapOpenError>
    where
        Self: Sized,
    {
        let rx_ring = NethunsRing::new(
            (opt.numblocks * opt.numpackets) as _,
            opt.packetsize as _,
        );
        
        let snaplen: u32;
        let magic: u32;
        
        let reader = if !writing_mode {
            let mut file = File::open(filename)?;
            let mut file_header = pcap_file_header {
                magic: 0,
                version_major: 0,
                version_minor: 0,
                thiszone: 0,
                sigfigs: 0,
                snaplen: 0,
                linktype: 0,
            };
            
            // Read PCAP file header
            let bytes =
                file.read(unsafe { any_as_u8_slice_mut(&mut file_header) })?;
            if bytes != mem::size_of::<pcap_file_header>() {
                return Err(NethunsPcapOpenError::PcapError(format!(
                    "unable to read pcap file header: file too short ({bytes})"
                )));
            }
            
            // Check if the file format is supported
            if file_header.magic != TCPDUMP_MAGIC
                && file_header.magic != KUZNETZOV_TCPDUMP_MAGIC
                && file_header.magic != NSEC_TCPDUMP_MAGIC
            {
                return Err(NethunsPcapOpenError::MagicNotSupported(
                    file_header.magic,
                ));
            }
            
            // Initialize fields for NethunsSocketPcap struct
            snaplen = cmp::min(file_header.snaplen, opt.packetsize);
            magic = file_header.magic;
            
            file
        } else {
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(filename)?;
            
            snaplen = opt.packetsize;
            magic = TCPDUMP_MAGIC;
            
            let file_header = pcap_file_header {
                magic,
                version_major: 2,
                version_minor: 4,
                thiszone: 0,
                sigfigs: 0,
                snaplen: 0xffff,
                linktype: 1, // DLT_EN10MB
            };
            
            let bytes = file.write(unsafe { any_as_u8_slice(&file_header) })?;
            if bytes != mem::size_of::<pcap_file_header>() {
                return Err(
                        NethunsPcapOpenError::PcapError(
                            format!(
                                "unable to write pcap file header: writen {bytes} bytes instead of {} bytes", mem::size_of::<pcap_file_header>()
                            )
                        )
                    );
            }
            
            file.flush()?;
            
            file
        };
        
        let base = NethunsSocketBase {
            opt,
            rx_ring: Some(rx_ring),
            ..Default::default()
        };
        
        Ok(NethunsSocketPcap {
            base,
            reader,
            snaplen,
            magic,
        })
    }
    
    
    fn read(&mut self) -> Result<RecvPacket, NethunsPcapReadError> {
        todo!()
    }
    
    
    fn write(&mut self) -> Result<(), String> {
        todo!()
    }
    
    
    fn store(&mut self) -> Result<(), String> {
        todo!()
    }
    
    
    fn rewind(&mut self) -> Result<(), String> {
        todo!()
    }
}


unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::core::slice::from_raw_parts(
        (p as *const T) as *const u8,
        ::core::mem::size_of::<T>(),
    )
}

unsafe fn any_as_u8_slice_mut<T: Sized>(p: &mut T) -> &mut [u8] {
    ::core::slice::from_raw_parts_mut(
        (p as *mut T) as *mut u8,
        ::core::mem::size_of::<T>(),
    )
}
