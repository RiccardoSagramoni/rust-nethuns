use crate::sockets::base::NethunsSocketBase;
use crate::sockets::types::NethunsPkthdrType;
use derivative::Derivative;
use derive_builder::Builder;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsCaptureDir {
    #[default]
    In,
    Out,
    InOut,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsCaptureMode {
    #[default]
    Default,
    SkbMode,
    DrvMode,
    ZeroCopy,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsSocketMode {
    #[default]
    RxTx,
    RxOnly,
    TxOnly,
}


#[repr(C)]
#[derive(Clone, Copy, Builder, Debug, Derivative, PartialEq, PartialOrd)]
#[derivative(Default)]
#[builder(pattern = "owned", default)]
pub struct NethunsSocketOptions {
    pub numblocks: libc::c_uint,
    pub numpackets: libc::c_uint,
    pub packetsize: libc::c_uint,
    pub timeout_ms: libc::c_uint,
    pub dir: NethunsCaptureDir,
    pub capture: NethunsCaptureMode,
    pub mode: NethunsSocketMode,
    pub promisc: bool,
    pub rxhash: bool,
    pub tx_qdisc_bypass: bool,
    #[derivative(Default(value="std::ptr::null()"))]
    pub xdp_prog: *const libc::c_char,
    #[derivative(Default(value="std::ptr::null()"))]
    pub xdp_prog_sec: *const libc::c_char,
    #[derivative(Default(value="std::ptr::null()"))]
    pub xsk_map_name: *const libc::c_char,
    pub reuse_maps: bool,
    #[derivative(Default(value="std::ptr::null()"))]
    pub pin_dir: *const libc::c_char,
}

#[derive(Clone, Copy, Builder, Debug, Default, PartialEq, PartialOrd)]
#[builder(pattern = "owned", default)]
pub struct NethunsStat {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_dropped: u64,
    pub rx_if_dropped: u64,
    pub rx_invalid: u64, // xdp only
    pub tx_invalid: u64, // xdp only
    pub freeze: u64,
}


#[derive(Builder, Debug, Default, PartialEq, PartialOrd)]
#[builder(pattern = "owned", default)]
pub struct NethunsPacket {
    pub payload: Vec<u8>,
    pub pkthdr: NethunsPkthdrType,
    pub sock: NethunsSocketBase,
    pub id: u64,
}


#[derive(Clone, Builder, Debug, Default, PartialEq, PartialOrd)]
#[builder(pattern = "owned", default)]
pub struct NethunsTimeval {
    tv_sec: u32,
    tv_usec: u32,
}


#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsQueue {
    Some(u32),
    #[default]
    Any,
}


#[cfg(test)]
mod tests {
    use is_trait::is_trait;
    
    
    #[test]
    fn check_send_trait() {
        assert!(is_trait!(super::NethunsCaptureDir, Send));
        assert!(is_trait!(super::NethunsCaptureMode, Send));
        assert!(is_trait!(super::NethunsSocketMode, Send));
        assert!(is_trait!(super::NethunsSocketOptions, Send));
        assert!(is_trait!(super::NethunsStat, Send));
        assert!(is_trait!(super::NethunsPacket, Send));
        assert!(is_trait!(super::NethunsTimeval, Send));
    }
    
    
    #[test]
    fn check_sync_trait() {
        assert!(is_trait!(super::NethunsCaptureDir, Sync));
        assert!(is_trait!(super::NethunsCaptureMode, Sync));
        assert!(is_trait!(super::NethunsSocketMode, Sync));
        assert!(is_trait!(super::NethunsSocketOptions, Sync));
        assert!(is_trait!(super::NethunsStat, Sync));
        assert!(is_trait!(super::NethunsPacket, Sync));
        assert!(is_trait!(super::NethunsTimeval, Sync));
    }
    
    
    #[test]
    fn test_nethuns_socket_options_builder() {
        let numblocks: u32 = 12;
        let opt1 = super::NethunsSocketOptionsBuilder::default()
            .numblocks(numblocks)
            .build()
            .unwrap();
        let mut opt2 = super::NethunsSocketOptions::default();
        opt2.numblocks = numblocks;
        assert_eq!(opt1, opt2);
    }
}
