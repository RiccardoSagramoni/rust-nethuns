use std::{cmp, slice};
use std::fs::File;
use std::rc::Rc;
use std::sync::atomic;

use pcap_parser::traits::PcapReaderIterator;
use pcap_parser::{LegacyPcapReader, PcapBlockOwned};

use crate::sockets::base::pcap::socket::NSEC_TCPDUMP_MAGIC;
use crate::sockets::base::pcap::NethunsSocketPcap;
use crate::sockets::base::{NethunsSocketBase, RecvPacket};
use crate::sockets::ring::NethunsRing;
use crate::sockets::PkthdrTrait;
use crate::types::NethunsSocketOptions;


pub type PcapReaderType = LegacyPcapReader<File>;


impl NethunsSocketPcap {
    /// TODO doc
    /// TODO better type error
    pub fn open(
        opt: NethunsSocketOptions,
        filename: &str,
        writing_mode: bool,
    ) -> Result<Self, String> {
        if writing_mode {
            return Err("[open] could not open pcap file for writing (use built-in pcap option)".to_owned());
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
        
        let mut reader =
            LegacyPcapReader::new(65536, File::open(filename).unwrap())
                .unwrap();
        let header = match reader.next() {
            Ok((_, block)) => match block {
                PcapBlockOwned::LegacyHeader(header) => header,
                _ => unreachable!(),
            },
            _ => panic!(),
        };
        
        Ok(NethunsSocketPcap {
            base,
            reader,
            snaplen,
            magic: header.magic_number,
        })
    }
    
    
    /// TODO doc
    /// TODO better type error
    pub fn read(&mut self) -> Result<RecvPacket, String> {
        let rx_ring = self.base.rx_ring.as_mut().unwrap();
        
        let caplen = self.base.opt.packetsize;
        let rc_slot = rx_ring.get_slot(rx_ring.rings.head());
        if rc_slot.borrow().inuse.load(atomic::Ordering::Acquire) != 0 {
            return Err("inuse slot".to_owned());
        }
        
        let pcap_packet = match self.reader.next() {
            Ok((_, block)) => match block {
                PcapBlockOwned::Legacy(packet) => packet,
                _ => unreachable!(),
            },
            _ => panic!(),
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
            unsafe {
                slice::from_raw_parts(slot.packet.as_ptr(), bytes as _)
            },
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
