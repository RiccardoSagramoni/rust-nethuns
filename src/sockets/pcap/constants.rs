//! Magic numbers for the supported pcap formats.

#![allow(dead_code)]

pub const TCPDUMP_MAGIC: u32 = 0xa1b2_c3d4;
pub const KUZNETZOV_TCPDUMP_MAGIC: u32 = 0xa1b2_cd34;
pub const FMESQUITA_TCPDUMP_MAGIC: u32 = 0xa1b2_34cd;
pub const NAVTEL_TCPDUMP_MAGIC: u32 = 0xa12b_3c4d;
pub const NSEC_TCPDUMP_MAGIC: u32 = 0xa1b2_3c4d;
