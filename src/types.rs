use derivative::Derivative;
use derive_builder::Builder;


#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsQueue {
    Some(u32),
    #[default]
    Any,
}


#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsCaptureDir {
    #[default]
    In,
    Out,
    InOut,
}


#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsCaptureMode {
    #[default]
    Default,
    SkbMode,
    DrvMode,
    ZeroCopy,
}


#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub enum NethunsSocketMode {
    #[default]
    RxTx,
    RxOnly,
    TxOnly,
}


/// Options for the nethuns socket.
#[derive(Builder, Clone, Debug, Derivative, PartialEq, PartialOrd)]
#[derivative(Default)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct NethunsStat {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_dropped: u64,
    pub rx_if_dropped: u64,
    /// xdp only
    pub rx_invalid: u64,
    /// xdp only
    pub tx_invalid: u64,
    pub freeze: u64,
}


#[cfg(test)]
mod tests {
    use is_trait::is_trait;
    
    
    #[test]
    fn assert_send_trait() {
        assert!(is_trait!(super::NethunsCaptureDir, Send));
        assert!(is_trait!(super::NethunsCaptureMode, Send));
        assert!(is_trait!(super::NethunsSocketMode, Send));
        assert!(is_trait!(super::NethunsSocketOptions, Send));
        assert!(is_trait!(super::NethunsStat, Send));
    }
    
    
    #[test]
    fn assert_sync_trait() {
        assert!(is_trait!(super::NethunsCaptureDir, Sync));
        assert!(is_trait!(super::NethunsCaptureMode, Sync));
        assert!(is_trait!(super::NethunsSocketMode, Sync));
        assert!(is_trait!(super::NethunsSocketOptions, Sync));
        assert!(is_trait!(super::NethunsStat, Sync));
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
