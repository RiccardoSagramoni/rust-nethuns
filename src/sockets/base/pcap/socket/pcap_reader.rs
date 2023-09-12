use std::fs::File;
use std::rc::Rc;
use std::sync::atomic;
use std::{cmp, slice};

use pcap_parser::traits::PcapReaderIterator;
use pcap_parser::{LegacyPcapReader, PcapBlockOwned};

use crate::sockets::base::pcap::socket::NSEC_TCPDUMP_MAGIC;
use crate::sockets::base::pcap::NethunsSocketPcap;
use crate::sockets::base::{NethunsSocketBase, RecvPacket};
use crate::sockets::errors::{NethunsPcapOpenError, NethunsPcapReadError};
use crate::sockets::ring::NethunsRing;
use crate::sockets::PkthdrTrait;
use crate::types::NethunsSocketOptions;


pub type PcapReaderType = LegacyPcapReader<File>;


impl NethunsSocketPcap {
    /// TODO doc
    pub fn open(
        opt: NethunsSocketOptions,
        filename: &str,
        writing_mode: bool,
    ) -> Result<Self, NethunsPcapOpenError> {
        if writing_mode {
            return Err(NethunsPcapOpenError::WriteModeNotSupported);
        }
        
        let snaplen = opt.packetsize;
        
        let rx_ring = NethunsRing::new(
            (opt.numblocks * opt.numpackets) as _,
            opt.packetsize as _,
        );
        let base = NethunsSocketBase {
            opt,
            rx_ring: Some(rx_ring),
            ..Default::default()
        };
        
        let mut reader = LegacyPcapReader::new(65536, File::open(filename)?)?;
        let header = match reader.next() {
            Ok((_, block)) => match block {
                PcapBlockOwned::LegacyHeader(header) => header,
                // The first read block should be the header of the pcap file
                _ => unreachable!(),
            },
            Err(e) => return Err(NethunsPcapOpenError::from(e)),
        };
        
        Ok(NethunsSocketPcap {
            base,
            reader,
            snaplen,
            magic: header.magic_number,
        })
    }
    
    
    /// TODO doc
    pub fn read(&mut self) -> Result<RecvPacket, NethunsPcapReadError> {
        let rx_ring = self
            .base
            .rx_ring
            .as_mut()
            .expect("[read] rx_ring should have been set during `open`");
        
        let caplen = self.base.opt.packetsize;
        let rc_slot = rx_ring.get_slot(rx_ring.rings.head());
        if rc_slot.borrow().inuse.load(atomic::Ordering::Acquire) != 0 {
            return Err(NethunsPcapReadError::InUse);
        }
        
        let pcap_packet = match self.reader.next() {
            Ok((_, block)) => match block {
                PcapBlockOwned::Legacy(packet) => packet,
                // The first read block should be the header of the pcap file
                _ => unreachable!(),
            },
            Err(e) => return Err(NethunsPcapReadError::from(e)),
        };
        
        let bytes = cmp::min(caplen, pcap_packet.caplen);
        let mut slot = rc_slot.borrow_mut();
        slot.pkthdr.tstamp_set_sec(pcap_packet.ts_sec);
        
        if self.magic == NSEC_TCPDUMP_MAGIC {
            slot.pkthdr.tstamp_set_nsec(pcap_packet.ts_usec)
        } else {
            slot.pkthdr.tstamp_set_usec(pcap_packet.ts_usec)
        }
        
        slot.pkthdr.set_len(pcap_packet.origlen);
        slot.pkthdr.set_snaplen(bytes);
        
        slot.packet.copy_from_slice(&pcap_packet.data[..bytes as _]);
        
        slot.inuse.store(1, atomic::Ordering::Release);
        rx_ring.rings.advance_head();
        
        let slot = rc_slot.borrow();
        
        Ok(RecvPacket::new(
            rx_ring.rings.head() as _,
            Box::new(slot.pkthdr),
            unsafe { slice::from_raw_parts(slot.packet.as_ptr(), bytes as _) },
            Rc::downgrade(&rc_slot),
        ))
    }
    
    
    pub fn write(&mut self) -> Result<(), String> {
        Err("Not supported".to_owned())
    }
    
    pub fn store(&mut self) -> Result<(), String> {
        Err("Not supported".to_owned())
    }
    
    pub fn rewind(&mut self) -> Result<(), String> {
        Err("Not supported".to_owned())
    }
}
