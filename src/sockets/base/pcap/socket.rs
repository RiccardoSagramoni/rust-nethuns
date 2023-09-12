use cfg_if::cfg_if;


const TCPDUMP_MAGIC: u32 = 0xa1b2c3d4;
const KUZNETZOV_TCPDUMP_MAGIC: u32 = 0xa1b2cd34;
const FMESQUITA_TCPDUMP_MAGIC: u32 = 0xa1b234cd;
const NAVTEL_TCPDUMP_MAGIC: u32 = 0xa12b3c4d;
const NSEC_TCPDUMP_MAGIC: u32 = 0xa1b23c4d;


cfg_if!(
    if #[cfg(feature="NETHUNS_USE_BUILTIN_PCAP_READER")] {
        mod builtin_reader;
        pub use builtin_reader::*;
    } else {
        mod pcap_reader;
        pub use pcap_reader::*;
    }
);
