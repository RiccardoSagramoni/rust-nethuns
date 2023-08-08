use std::sync::atomic::Ordering;

use c_netmap_wrapper::macros::netmap_rxring;
use c_netmap_wrapper::nmport::NmPortDescriptor;
use c_netmap_wrapper::ring::NetmapRing;

use crate::sockets::errors::NethunsRecvError;
use crate::sockets::ring::NethunsRing;


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
/// - `d` - A mutable reference to the `NmPortDescriptor` representing the Netmap port.
///
/// # Returns
///
/// - `Ok(NetmapRing)` - If a non-empty RX ring is found, it returns the corresponding `NetmapRing`.
/// - `Err(NethunsRecvError::FrameworkError)` - If `netmap_rxring` returns a null pointer.
/// - `Err(NethunsRecvError::NoPacketsAvailable)` - If all RX rings are empty, and the search fails.
///
/// # Safety
///
/// This function makes use of unsafe code due to the interaction with the Netmap C API
/// through the `netmap_rxring` function.
/// Be sure that the Netmap port descriptor is properly initialized.
pub fn non_empty_rx_ring(
    d: &mut NmPortDescriptor,
) -> Result<NetmapRing, NethunsRecvError> {
    let mut ri = d.cur_rx_ring;
    
    loop {
        // Compute current ring to use
        let ring =
            NetmapRing::try_new(unsafe { netmap_rxring(d.nifp, ri as usize) })
                .map_err(NethunsRecvError::FrameworkError)?;
        
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


/// TODO
#[inline(always)]
pub fn nethuns_send_slot(
    tx_ring: &NethunsRing,
    pktid: u64,
    len: usize,
) -> bool {
    let rc_slot = tx_ring.get_slot(pktid as usize);
    let mut slot = rc_slot.borrow_mut();
    if slot.inuse.load(Ordering::Acquire) {
        return false;
    }
    slot.len = len as i32;
    slot.inuse.store(true, Ordering::Release);
    true
}
