//! Constants for the bindings with the netmap C library.
//!
//!
//! # NIOCTXSYNC and NIOCRXSYNC
//! The ioctl commands to sync TX/RX netmap rings.
//!
//! NIOCTXSYNC, NIOCRXSYNC synchronize tx or rx queues,
//! whose identity is set in NETMAP_REQ_REGISTER through nr_ringid.
//! These are non blocking and take no argument.

/// sync tx queues
pub const NIOCTXSYNC: u64 = uapi::_IO('i' as _, 148_u64);
/// sync rx queues
pub const NIOCRXSYNC: u64 = uapi::_IO('i' as _, 149_u64);
