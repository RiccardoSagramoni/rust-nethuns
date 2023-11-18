//! Magic numbers for the supported pcap formats.

#![allow(dead_code)]

pub const TCPDUMP_MAGIC: u32 = 0xa1b2c3d4;
pub const KUZNETZOV_TCPDUMP_MAGIC: u32 = 0xa1b2cd34;
pub const FMESQUITA_TCPDUMP_MAGIC: u32 = 0xa1b234cd;
pub const NAVTEL_TCPDUMP_MAGIC: u32 = 0xa12b3c4d;
pub const NSEC_TCPDUMP_MAGIC: u32 = 0xa1b23c4d;
