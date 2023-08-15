use std::mem;

use crate::{types::NethunsQueue, NethunsSocket};


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
        socket.socket_base().devname.to_str().ok(),
        socket.socket_base().queue,
    )
}


/// Compute the closest power of 2 larger or equal than `x`
#[inline(always)]
pub(crate) fn nethuns_lpow2(x: usize) -> usize {
    if x != 0 && (x & (x - 1)) == 0 {
        x
    } else {
        1 << (mem::size_of::<usize>() * 8 - x.leading_zeros() as usize)
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn lpow2() {
        assert_eq!(super::nethuns_lpow2(0), 1);
        assert_eq!(super::nethuns_lpow2(1), 1);
        assert_eq!(super::nethuns_lpow2(2), 2);
        assert_eq!(super::nethuns_lpow2(5), 8);
        assert_eq!(super::nethuns_lpow2(12), 16);
        assert_eq!(super::nethuns_lpow2(16), 16);
        assert_eq!(super::nethuns_lpow2(30), 32);
    }
}
