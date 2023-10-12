pub(crate) mod circular_buffer;
pub(crate) mod send_rc;


use std::mem;

use crate::sockets::ring::NethunsRingSlot;
use crate::sockets::NethunsSocket;
use crate::types::NethunsQueue;

use self::send_rc::SendRc;


/// Get full device name, taking into account
/// both the real device name and the queue.
#[inline(always)]
pub fn nethuns_dev_queue_name(
    dev: Option<&str>,
    queue: NethunsQueue,
) -> String {
    match dev {
        None => "unspec".to_owned(),
        Some(dev) => match queue {
            NethunsQueue::Some(idx) => {
                format!("{}:{}", dev, idx)
            }
            NethunsQueue::Any => dev.to_owned(),
        },
    }
}


/// Get the name of the device bounded to the socket.
#[inline(always)]
pub fn nethuns_device_name(socket: &dyn NethunsSocket) -> String {
    nethuns_dev_queue_name(
        socket.base().devname().to_str().ok(),
        socket.base().get_queue(),
    )
}


/// Bind the lifetime of a packet to the corresponding slot.
/// This is **very unsafe**. Be careful!
///
/// # Safety
/// This function assumes that the following conditions hold:
/// * `pkt` is a slice of the buffer contained in the specified slot.
/// * `pkt` is valid as long as `slot` is valid.
/// * the content of `pkt` is immutable as long as `pkt` is valid.
#[inline(always)]
pub(crate) unsafe fn bind_packet_lifetime_to_slot<'a>(
    pkt: &[u8],
    _slot: &'a SendRc<NethunsRingSlot>,
) -> &'a [u8] {
    mem::transmute(pkt)
}


#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn test_nethuns_dev_queue_name() {
        assert_eq!(
            nethuns_dev_queue_name(None, NethunsQueue::Some(123)),
            "unspec".to_owned(),
        );
        assert_eq!(
            nethuns_dev_queue_name(None, NethunsQueue::Any),
            "unspec".to_owned(),
        );
        assert_eq!(
            nethuns_dev_queue_name(Some("eth0"), NethunsQueue::Some(123)),
            "eth0:123".to_owned(),
        );
        assert_eq!(
            nethuns_dev_queue_name(Some("eth0"), NethunsQueue::Any),
            "eth0".to_owned(),
        );
    }
}
