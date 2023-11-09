pub(crate) mod circular_buffer;
pub(crate) mod hybrid_rc;

use hybrid_rc::state_trait::RcState;

use crate::sockets::NethunsSocket;
use crate::types::NethunsQueue;


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
pub fn nethuns_device_name<State: RcState>(
    socket: &NethunsSocket<State>,
) -> String {
    nethuns_dev_queue_name(
        socket.base().devname().to_str().ok(),
        socket.base().get_queue(),
    )
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
