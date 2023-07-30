use crate::sockets::base::NethunsSocketBase;
use crate::sockets::types::NethunsPkthdrType;
use derive_builder::Builder;
use derive_new::new;


#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsCaptureDir {
    #[default]
    In,
    Out,
    InOut,
}


#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsCaptureMode {
    #[default]
    Default,
    SkbMode,
    DrvMode,
    ZeroCopy,
}


#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsSocketMode {
    #[default]
    RxTx,
    RxOnly,
    TxOnly,
}


#[derive(Clone, Builder, Debug, Default, new, PartialEq, PartialOrd)]
#[builder(pattern = "owned", default)]
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
    // TODO xdp-only fields
}


#[derive(Clone, Builder, Debug, Default, new, PartialEq, PartialOrd)]
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


#[derive(Builder, Debug, Default, new, PartialEq, PartialOrd)]
#[builder(pattern = "owned", default)]
pub struct NethunsPacket {
    pub payload: Vec<u8>,
    pub pkthdr: NethunsPkthdrType,
    pub sock: NethunsSocketBase,
    pub id: u64,
}


#[derive(Clone, Builder, Debug, Default, new, PartialEq, PartialOrd)]
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