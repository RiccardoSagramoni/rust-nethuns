//! This module contains the implementation of [`NethunsSocketPcapInner`]
//! when the default pcap reader is requested
//! (i.e. `NETHUNS_USE_BUILTIN_PCAP_READER` feature is **not** enabled).

use std::cmp;
use std::fs::File;
use std::sync::atomic;

use pcap_parser::traits::PcapReaderIterator;
use pcap_parser::{LegacyPcapReader, PcapBlockOwned, PcapError};

use crate::misc::hybrid_rc::state::{Local, Shared};
use crate::misc::hybrid_rc::state_trait::RcState;
use crate::sockets::base::{InnerRecvData, NethunsSocketBase, RecvPacketData};
use crate::sockets::errors::{
    NethunsPcapOpenError, NethunsPcapReadError, NethunsPcapRewindError,
    NethunsPcapStoreError, NethunsPcapWriteError,
};
use crate::sockets::ring::{NethunsRing, RingSlotStatus};
use crate::sockets::PkthdrTrait;
use crate::types::NethunsSocketOptions;

use super::constants::NSEC_TCPDUMP_MAGIC;
use super::{
    nethuns_pcap_pkthdr, LocalReadSocketPcapTrait, NethunsSocketPcapInner,
    NethunsSocketPcapTrait, SharedReadSocketPcapTrait,
};


// Define the type of the pcap reader
pub type PcapReaderType = LegacyPcapReader<File>;


impl<State: RcState> NethunsSocketPcapTrait<State>
    for NethunsSocketPcapInner<State>
{
    fn open(
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
            Ok((offset, block)) => match block {
                PcapBlockOwned::LegacyHeader(header) => {
                    let header = header.clone();
                    reader.consume(offset);
                    header
                }
                // The first read block should be the header of the pcap file
                _ => unreachable!(),
            },
            Err(e) => return Err(NethunsPcapOpenError::from(e)),
        };
        
        Ok(NethunsSocketPcapInner {
            base,
            reader,
            snaplen,
            magic: header.magic_number,
        })
    }
    
    
    fn write(
        &mut self,
        _header: &nethuns_pcap_pkthdr,
        _packet: &[u8],
    ) -> Result<usize, NethunsPcapWriteError> {
        Err(NethunsPcapWriteError::NotSupported)
    }
    
    
    fn store(
        &mut self,
        _pkthdr: &dyn PkthdrTrait,
        _packet: &[u8],
    ) -> Result<u32, NethunsPcapStoreError> {
        Err(NethunsPcapStoreError::NotSupported)
    }
    
    
    fn rewind(&mut self) -> Result<u64, NethunsPcapRewindError> {
        Err(NethunsPcapRewindError::NotSupported)
    }
}


impl<State: RcState> NethunsSocketPcapInner<State> {
    fn inner_read(
        &mut self,
    ) -> Result<InnerRecvData<State>, NethunsPcapReadError> {
        let rx_ring = self
            .base
            .rx_ring
            .as_mut()
            .expect("[read] rx_ring should have been set during `open`");
        
        let caplen = self.base.opt.packetsize;
        let head_idx = rx_ring.head();
        let slot = rx_ring.get_slot_mut(head_idx);
        if slot.status.load(atomic::Ordering::Acquire) != RingSlotStatus::Free {
            return Err(NethunsPcapReadError::InUse);
        }
        
        let bytes: u32;
        loop {
            match self.reader.next() {
                Ok((offset, block)) => match block {
                    PcapBlockOwned::Legacy(packet) => {
                        bytes = cmp::min(caplen, packet.caplen);
                        
                        slot.pkthdr.tstamp_set_sec(packet.ts_sec);
                        
                        if self.magic == NSEC_TCPDUMP_MAGIC {
                            slot.pkthdr.tstamp_set_nsec(packet.ts_usec)
                        } else {
                            slot.pkthdr.tstamp_set_usec(packet.ts_usec)
                        }
                        
                        slot.pkthdr.set_len(packet.origlen);
                        slot.pkthdr.set_snaplen(bytes);
                        
                        slot.packet.copy_from_slice(&packet.data[..bytes as _]);
                        self.reader.consume(offset);
                        break;
                    }
                    // We should have read a packet
                    _ => unreachable!(),
                },
                Err(PcapError::Incomplete) => {
                    self.reader.refill()?;
                    continue;
                }
                Err(e) => return Err(NethunsPcapReadError::from(e)),
            };
        }
        
        slot.status
            .store(RingSlotStatus::InUse, atomic::Ordering::Release);
        
        rx_ring.rings_mut().advance_head();
        
        let slot = rx_ring.get_slot(head_idx);
        
        Ok(InnerRecvData {
            id: rx_ring.rings().head() as _,
            pkthdr: &slot.pkthdr,
            buffer: &slot.packet[..bytes as _],
            slot_status_flag: &slot.status,
        })
    }
}

impl LocalReadSocketPcapTrait for NethunsSocketPcapInner<Local> {
    fn read(&mut self) -> Result<RecvPacketData<Local>, NethunsPcapReadError> {
        let packet = self.inner_read()?;
        Ok(RecvPacketData::new(
            packet.id,
            packet.pkthdr,
            packet.buffer,
            packet.slot_status_flag.clone(),
        ))
    }
}

impl SharedReadSocketPcapTrait for NethunsSocketPcapInner<Shared> {
    fn read(&mut self) -> Result<RecvPacketData<Shared>, NethunsPcapReadError> {
        let packet = self.inner_read()?;
        Ok(RecvPacketData::new(
            packet.id,
            packet.pkthdr,
            packet.buffer,
            packet.slot_status_flag.clone(),
        ))
    }
}
