//! Module containing some helper functions for [super] module

use std::ptr::NonNull;

use c_netmap_wrapper::macros::netmap_rxring;
use c_netmap_wrapper::nmport::NmPortDescriptor;
use c_netmap_wrapper::ring::NetmapRing;

use crate::sockets::errors::NethunsRecvError;


/// Finds the first non-empty RX ring within the given Netmap port descriptor.
///
/// This function searches for a non-empty RX (receive) ring within the Netmap
/// port descriptor `d`. It iterates through the available RX rings, starting
/// from the `cur_rx_ring` field of the descriptor, and wraps around if necessary.
/// The function returns the first non-empty ring found, updating the `cur_rx_ring`
/// field of the descriptor to point to this ring.
///
/// # Arguments
///
/// * `d` - A mutable reference to the `NmPortDescriptor` representing the Netmap port.
///
/// # Returns
///
/// * `Ok(NetmapRing)` - If a non-empty RX ring is found, it returns the corresponding `NetmapRing`.
/// * `Err(NethunsRecvError::FrameworkError)` - If `netmap_rxring` returns a null pointer.
/// * `Err(NethunsRecvError::NoPacketsAvailable)` - If all RX rings are empty, and the search fails.
///
/// # Safety
///
/// This function makes use of unsafe code due to the interaction with the Netmap C API
/// through the `netmap_rxring` function.
/// Be sure that the Netmap port descriptor is properly initialized.
pub(super) fn non_empty_rx_ring(
    d: &mut NmPortDescriptor,
) -> Result<NetmapRing, NethunsRecvError> {
    let mut ri = d.cur_rx_ring;
    
    loop {
        // Compute current ring to use
        let ring = NetmapRing::new(
            NonNull::new(unsafe { netmap_rxring(d.nifp, ri as _) }).ok_or(
                NethunsRecvError::FrameworkError(
                    "[non_empty_rx_ring] netmap_rxring returned null"
                        .to_owned(),
                ),
            )?,
        );
        
        // Check if the ring contains some received packets
        if ring.cur != ring.tail {
            // Update the last RX ring used and return the ring
            d.cur_rx_ring = ri;
            return Ok(ring);
        }
        
        // Move the search to the next ring
        ri += 1;
        if ri > d.last_rx_ring {
            ri = d.first_rx_ring;
        }
        
        if ri == d.cur_rx_ring {
            // All rings are empty: search failed
            return Err(NethunsRecvError::NoPacketsAvailable);
        }
    }
}


/// Add the id of a newly available ring slot
/// to the list of currently available slots.
///
/// This should be passed to [`crate::sockets::ring::nethuns_ring_free_slots`] as
/// *free_macro* parameter.
///
/// # Arguments
/// * `s` - the nethuns socket
/// * `slot` - the newly available ring slot
macro_rules! nethuns_blocks_free {
    ($s: expr, $slot: expr) => {
        $s.free_ring.as_mut().unwrap().push_unchecked($slot.pkthdr.buf_idx);
    };
}
pub(super) use nethuns_blocks_free;


/// Get a raw pointer to the buffer which contains the packet,
/// inside a specific ring slot.
///
/// # Arguments
/// * `$some_ring`: a NetmapRing object
/// * `$tx_ring`: a NethunsRing object
/// * `$pktid`: the ring slot ID
/// FIXME better doc
///
/// # Returns
/// A `*mut u8` raw pointer pointing to the requested buffer
macro_rules! nethuns_get_buf_addr_netmap {
    ($some_ring: expr, $tx_ring: expr, $pktid: expr) => {
        netmap_buf(
            $some_ring,
            $tx_ring.get_slot($pktid).borrow().pkthdr.buf_idx as _,
        ) as *mut u8
    };
}
pub(super) use nethuns_get_buf_addr_netmap;
