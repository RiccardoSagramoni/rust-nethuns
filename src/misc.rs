mod circular_buffer;

pub(crate) use circular_buffer::*;

use crate::types::NethunsQueue;
use crate::NethunsSocket;


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
