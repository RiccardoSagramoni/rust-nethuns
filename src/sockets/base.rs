use std::ffi::CString;
use std::fmt::{self, Debug, Display};
use std::marker::PhantomData;
use std::mem;
use std::sync::atomic;

use derivative::Derivative;
use getset::{CopyGetters, Getters, Setters};

use crate::types::{NethunsFilter, NethunsQueue, NethunsSocketOptions};

// TODO
// use super::pcap::NethunsSocketPcap;
use super::api::Pkthdr;
use super::ring::{AtomicRingSlotStatus, NethunsRing, RingSlotStatus};
use super::PkthdrTrait;


/// Base structure for a `NethunsSocket`.
///
/// This data structure is common to all the implementation of a "nethuns socket",
/// for the supported underlying I/O frameworks. Thus, it's independent from
/// low-level implementation of the sockets.
#[derive(Default, Derivative, Getters, Setters, CopyGetters)]
#[derivative(Debug)]
#[getset(get = "pub")]
pub struct NethunsSocketBase {
    /// Configuration options
    pub(super) opt: NethunsSocketOptions,
    
    /// Rings used for transmission
    pub(super) tx_ring: Option<NethunsRing>,
    
    /// Rings used for reception
    pub(super) rx_ring: Option<NethunsRing>,
    
    /// Name of the binded device
    pub(super) devname: CString,
    
    /// Queue binded to the socket
    #[getset(get_copy = "pub with_prefix")]
    pub(super) queue: NethunsQueue,
    
    /// Index of the interface
    #[getset(get_copy = "pub with_prefix")]
    pub(super) ifindex: libc::c_int,
    
    /// Closure used for filtering received packets.
    #[derivative(Debug = "ignore")]
    #[getset(set = "pub")]
    pub(super) filter: Option<Box<NethunsFilter>>,
    // errbuf removed => use Result as return type
    // filter_ctx removed => use closures with move semantics
}


//


/// Packet received when calling [`NethunsSocket::recv()`](crate::sockets::NethunsSocket::recv)
/// or [`NethunsSocketPcap::read()`](crate::sockets::pcap::NethunsSocketPcap::read).
///
/// The struct contains a [`PhantomData`] marker associated with the socket itself,
/// so that the `RecvPacket` item is valid as long as the socket is alive.
#[derive(Debug)]
pub struct RecvPacket<'a, T> {
    data: RecvPacketData<'a>,
    
    phantom_data: PhantomData<&'a T>,
}


impl<'a, T> RecvPacket<'a, T> {
    pub(super) fn new<'b>(
        data: RecvPacketData<'b>,
        phantom_data: PhantomData<&'a T>,
    ) -> Self {
        let data: RecvPacketData<'a> = unsafe {
            // [SAFETY] As long as the socket is alive, the references
            // to the data are valid
            RecvPacketData {
                id: data.id,
                pkthdr: mem::transmute(data.pkthdr),
                buffer: mem::transmute(data.buffer),
                slot_status_flag: mem::transmute(data.slot_status_flag),
            }
        };
        RecvPacket { data, phantom_data }
    }
    
    #[inline(always)]
    pub fn id(&self) -> usize {
        self.data.id
    }
    
    #[inline(always)]
    pub fn pkthdr(&self) -> &dyn PkthdrTrait {
        self.data.pkthdr
    }
    
    #[inline(always)]
    pub fn buffer(&self) -> &[u8] {
        self.data.buffer
    }
}


impl<T> Display for RecvPacket<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{\n    id: {},\n    pkthdr: {:?},\n    buffer: {:?}\n}}",
            self.id(),
            self.pkthdr(),
            self.buffer()
        )
    }
}


//


/// Packet received when calling [`NethunsSocket::recv()`](crate::sockets::NethunsSocket::recv)
/// or [`NethunsSocketPcap::read()`](crate::sockets::pcap::NethunsSocketPcap::read)
/// with static lifetime.
///
/// It **must** be wrapped inside `RecvPacket` struct before being handed to the user.
#[derive(Debug)]
pub(super) struct RecvPacketData<'a> {
    id: usize,
    pkthdr: &'a dyn PkthdrTrait,
    
    buffer: &'a [u8],
    
    /// Reference used to set the status flag of the corresponding ring slot
    /// to `Free` when the `RecPacketData` is dropped.
    slot_status_flag: &'a AtomicRingSlotStatus,
}

impl<'a> RecvPacketData<'a> {
    pub fn new(
        id: usize,
        pkthdr: &'a Pkthdr,
        buffer: &'a [u8],
        slot_status_flag: &'a AtomicRingSlotStatus,
    ) -> Self {
        Self {
            id,
            pkthdr,
            buffer,
            slot_status_flag,
        }
    }
}

impl Drop for RecvPacketData<'_> {
    /// Release the buffer by resetting the status flag of
    /// the corresponding ring slot.
    fn drop(&mut self) {
        self.slot_status_flag
            .store(RingSlotStatus::Free, atomic::Ordering::Release);
    }
}
