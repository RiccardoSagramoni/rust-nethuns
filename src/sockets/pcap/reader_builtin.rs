//! This module contains the implementation of [`NethunsSocketPcapInner`]
//! when the built-in pcap reader is requested
//! (i.e. `NETHUNS_USE_BUILTIN_PCAP_READER` feature is enabled).

use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::sync::atomic::Ordering;
use std::{cmp, mem};

use crate::sockets::base::{NethunsSocketBase, RecvPacketData};
use crate::sockets::errors::{
    NethunsPcapOpenError, NethunsPcapReadError, NethunsPcapRewindError,
    NethunsPcapStoreError, NethunsPcapWriteError,
};
use crate::sockets::ring::{NethunsRing, RingSlotStatus};
use crate::sockets::PkthdrTrait;
use crate::types::NethunsSocketOptions;

use super::constants::{
    KUZNETZOV_TCPDUMP_MAGIC, NSEC_TCPDUMP_MAGIC, TCPDUMP_MAGIC,
};
use super::{
    nethuns_pcap_patched_pkthdr, nethuns_pcap_pkthdr, nethuns_pcap_timeval,
    NethunsSocketPcapInner, NethunsSocketPcapTrait,
};


// Define the type of the pcap reader
pub type PcapReaderType = File;


impl NethunsSocketPcapTrait for NethunsSocketPcapInner {
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
            file.read_exact(unsafe { any_as_u8_slice_mut(&mut file_header) })?;
            
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
            // Create a new file in pcap format and
            // write the file header according to the TCPDUMP standard.
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
            
            file.write_all(unsafe { any_as_u8_slice(&file_header) })?;
            file.flush()?;
            file
        };
        
        let base = NethunsSocketBase {
            opt,
            rx_ring: Some(rx_ring),
            ..Default::default()
        };
        
        Ok(NethunsSocketPcapInner {
            base,
            reader,
            snaplen,
            magic,
        })
    }
    
    
    fn read(&mut self) -> Result<RecvPacketData, NethunsPcapReadError> {
        let rx_ring =
            self.base.rx_ring.as_mut().expect(
                "[pcap_read] rx_ring should have been set during `open`",
            );
        
        let caplen = self.base.opt.packetsize;
        let head_idx = rx_ring.head();
        let slot = rx_ring.get_slot_mut(head_idx);
        if slot.status.load(Ordering::Acquire) != RingSlotStatus::Free {
            return Err(NethunsPcapReadError::InUse);
        }
        
        // Read a new packet (header + payload) from the file
        let mut header = nethuns_pcap_patched_pkthdr::default();
        let header_slice = if self.magic == KUZNETZOV_TCPDUMP_MAGIC {
            unsafe { any_as_u8_slice_mut(&mut header) }
        } else {
            unsafe { any_as_u8_slice_mut(&mut header.hdr) }
        };
        
        self.reader.read_exact(header_slice)?;
        
        let bytes = cmp::min(caplen, header.hdr.caplen);
        
        self.reader.read_exact(&mut slot.packet[..bytes as _])?;
        
        // Store the information related to the new packet
        // in a free ring slot of the base nethuns socket
        slot.pkthdr.tstamp_set_sec(header.hdr.ts.tv_sec as _);
        
        if self.magic == NSEC_TCPDUMP_MAGIC {
            slot.pkthdr.tstamp_set_nsec(header.hdr.ts.tv_usec as _);
        } else {
            slot.pkthdr.tstamp_set_usec(header.hdr.ts.tv_usec as _);
        }
        
        slot.pkthdr.set_len(header.hdr.len);
        slot.pkthdr.set_snaplen(bytes);
        
        if header.hdr.caplen > caplen {
            let skip = header.hdr.caplen as i64 - caplen as i64;
            self.reader.seek(SeekFrom::Current(skip))?;
        }
        
        slot.status.store(RingSlotStatus::InUse, Ordering::Release);
        
        rx_ring.rings_mut().advance_head();
        
        let recv_packet = {
            // IMPORTANT!! slot MUST be an **immutable** reference,
            // otherwise the Rust memory model rules will be broken.
            let slot = rx_ring.get_slot(head_idx);
            
            RecvPacketData::new(
                rx_ring.head() as _,
                &slot.pkthdr,
                &slot.packet[..bytes as _],
                &slot.status,
            )
        };
        
        Ok(recv_packet)
    }
    
    
    fn write(
        &mut self,
        header: &nethuns_pcap_pkthdr,
        packet: &[u8],
    ) -> Result<usize, NethunsPcapWriteError> {
        // Write the header + packet into the file
        self.reader.write_all(unsafe { any_as_u8_slice(header) })?;
        self.reader.write_all(packet)?;
        self.reader.flush()?;
        Ok(packet.len())
    }
    
    
    fn store(
        &mut self,
        pkthdr: &dyn PkthdrTrait,
        packet: &[u8],
    ) -> Result<u32, NethunsPcapStoreError> {
        // Build a packet header for the pcap format from the
        // header of the original packet
        let has_vlan_offload = pkthdr.offvlan_tpid();
        let header = nethuns_pcap_pkthdr {
            ts: nethuns_pcap_timeval {
                tv_sec: pkthdr.tstamp_sec() as _,
                tv_usec: pkthdr.tstamp_usec() as _,
            },
            caplen: cmp::min(
                packet.len() as _,
                pkthdr.snaplen() + 4 * has_vlan_offload as u32,
            ),
            len: pkthdr.len() + 4 * has_vlan_offload as u32,
        };
        
        // Write the packet header
        self.reader.write_all(unsafe { any_as_u8_slice(&header) })?;
        
        let mut clen: u32 = header.caplen;
        
        // Write the packet payload
        if has_vlan_offload != 0 {
            let h8021q: [u16; 2] =
                [pkthdr.offvlan_tpid().to_be(), pkthdr.offvlan_tci().to_be()];
            self.reader.write_all(&packet[..12])?;
            self.reader.write_all(unsafe { any_as_u8_slice(&h8021q) })?;
            clen = header.caplen - 16;
            self.reader.write_all(&packet[12..(clen + 12) as _])?;
        } else {
            self.reader.write_all(&packet[..header.caplen as _])?;
        }
        
        self.reader.flush()?;
        Ok(clen)
    }
    
    
    fn rewind(&mut self) -> Result<u64, NethunsPcapRewindError> {
        // Rewind the cursor of the file to the start of the file
        self.reader
            .seek(SeekFrom::Start(mem::size_of::<pcap_file_header>() as _))
            .map_err(NethunsPcapRewindError::from)
    }
}


/// Convert any reference to a slice of `u8`.
///
/// # Safety
/// This function is unsafe because any padding bytes in the struct
/// may be uninitialized memory (giving undefined behavior).
/// The struct should have been created with the `#[repr(C)]` attribute
/// for safe behavior and compatibility with C.
unsafe fn any_as_u8_slice<'a, T: Sized>(p: &'a T) -> &[u8] {
    core::slice::from_raw_parts::<'a, _>(
        (p as *const T) as *const u8,
        mem::size_of::<T>(),
    )
}

/// Convert any reference to a mutable slice of `u8`.
///
/// # Safety
/// This function is unsafe because any padding bytes in the struct
/// may be uninitialized memory (giving undefined behavior).
/// The struct should have been created with the `#[repr(C)]` attribute
/// for safe behavior and compatibility with C.
unsafe fn any_as_u8_slice_mut<'a, T: Sized>(p: &'a mut T) -> &mut [u8] {
    core::slice::from_raw_parts_mut::<'a, _>(
        (p as *mut T) as *mut u8,
        mem::size_of::<T>(),
    )
}


/// Header of a libpcap dump file.
///
/// The first record in the file contains saved values for some of the flags used
/// in the printout phases of tcpdump.
/// Many fields here are 32 bit ints so compilers won't
/// insert unwanted padding; these files need to be interchangeable
/// across architectures.
#[allow(non_camel_case_types)]
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
struct pcap_file_header {
    magic: u32,
    version_major: libc::c_ushort,
    version_minor: libc::c_ushort,
    thiszone: u32,
    sigfigs: u32,
    snaplen: u32,
    linktype: u32,
}
