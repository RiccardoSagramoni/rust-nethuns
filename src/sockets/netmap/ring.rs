use c_netmap_wrapper::macros::netmap_rxring;
use c_netmap_wrapper::nmport::NmPortDescriptor;
use c_netmap_wrapper::ring::NetmapRing;

use crate::sockets::errors::NethunsRecvError;


/// Search for a RX ring which contains packet to be received
pub fn non_empty_rx_ring(d: &mut NmPortDescriptor) -> Result<NetmapRing, NethunsRecvError> {
    let mut ri = d.cur_rx_ring;
    
    loop {
        // Compute current ring to use
        let ring =
            NetmapRing::try_new(unsafe { netmap_rxring(d.nifp, ri as usize) }).map_err(|e| NethunsRecvError::NethunsError(e))?;
        
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
