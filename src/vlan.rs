use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};
use etherparse::{Ethernet2Header, SingleVlanHeader};

use crate::define::{NETHUNS_ETH_P_8021AD, NETHUNS_ETH_P_8021Q};
use crate::PkthdrTrait;


/// VLAN identifier
#[inline(always)]
pub fn nethuns_vlan_vid(tci: u16) -> u16 {
    tci & ((1 << 13) - 1)
}


/// Priority code point
#[inline(always)]
pub fn nethuns_vlan_pcp(tci: u16) -> u16 {
    (tci >> 13) & 0x7
}


/// Drop eligible indicator
#[inline(always)]
pub fn nethuns_vlan_dei(tci: u16) -> u16 {
    (tci >> 12) & 1
}


/// Tag protocol identifier
#[inline(always)]
pub fn nethuns_vlan_tpid(payload: &[u8]) -> u16 {
    match Ethernet2Header::from_slice(payload) {
        Ok(eth) => {
            if eth.0.ether_type == NETHUNS_ETH_P_8021Q
                || eth.0.ether_type == NETHUNS_ETH_P_8021AD
            {
                eth.0.ether_type
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}


/// Tag control information
#[inline(always)]
pub fn nethuns_vlan_tci(payload: &[u8]) -> u16 {
    match SingleVlanHeader::from_slice(payload) {
        Ok(vlan) => {
            if vlan.0.ether_type == NETHUNS_ETH_P_8021Q
                || vlan.0.ether_type == NETHUNS_ETH_P_8021AD
            {
                u16::from_be(
                    Cursor::new(
                        vlan.0
                            .to_bytes()
                            .expect("Unable to convert VLAN header to bytes"),
                    )
                    .read_u16::<BigEndian>()
                    .unwrap_or(0),
                )
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}


/// Tag protocol identifier for nethuns socket
#[inline(always)]
pub fn nethuns_vlan_tpid_(hdr: &dyn PkthdrTrait, payload: &[u8]) -> u16 {
    let tpid = hdr.offvlan_tpid();
    if tpid != 0 {
        tpid
    } else {
        nethuns_vlan_tpid(payload)
    }
}


/// Tag control information for nethuns socket
pub fn nethuns_vlan_tci_(
    hdr: &dyn PkthdrTrait,
    payload: &[u8],
) -> u16 {
    if hdr.offvlan_tpid() != 0 {
        hdr.offvlan_tci()
    } else {
        nethuns_vlan_tci(payload)
    }
}
