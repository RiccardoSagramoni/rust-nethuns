use getset::CopyGetters;

use crate::sockets::PkthdrTrait;


/// Closure type for the filtering of received packets.
/// Returns true if the packet should be received, false if it should be discarded.
pub type NethunsFilter = dyn Fn(&dyn PkthdrTrait, &[u8]) -> bool + Send;


/// Enum for specifying which queue of the device should be used
/// for capturing packets.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub enum NethunsQueue {
    #[default]
    Any,
    Some(u32),
}


/// Enum for specifying the direction for capturing packets.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub enum NethunsCaptureDir {
    In,
    Out,
    #[default]
    InOut,
}


/// Enum for specifying the mode for capturing packets.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub enum NethunsCaptureMode {
    #[default]
    Default,
    SkbMode,
    DrvMode,
    ZeroCopy,
}


/// Enum for specifying the mode (rx/tx) for the nethuns socket.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub enum NethunsSocketMode {
    #[default]
    RxTx,
    RxOnly,
    TxOnly,
}


/// Options for the nethuns socket.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct NethunsSocketOptions {
    pub numblocks: u32,
    pub numpackets: u32,
    pub packetsize: u32,
    pub timeout_ms: u32,
    pub dir: NethunsCaptureDir,
    pub capture: NethunsCaptureMode,
    pub mode: NethunsSocketMode,
    pub promisc: bool,
    pub rxhash: bool,
    pub tx_qdisc_bypass: bool,
    
    /// xdp only
    pub xdp_prog: Option<String>,
    /// xdp only   
    pub xdp_prog_sec: Option<String>,
    /// xdp only
    pub xsk_map_name: Option<String>,
    /// xdp only
    pub reuse_maps: Option<bool>,
    /// xdp only
    pub pin_dir: Option<String>,
}


/// Statistics for the nethuns socket.
#[derive(
    Clone, Copy, CopyGetters, Debug, Default, PartialEq, PartialOrd, Eq, Ord,
)]
#[getset(get_copy = "pub")]
pub struct NethunsStat {
    rx_packets: u64,
    tx_packets: u64,
    rx_dropped: u64,
    rx_if_dropped: u64,
    /// xdp only
    rx_invalid: u64,
    /// xdp only
    tx_invalid: u64,
    freeze: u64,
}
