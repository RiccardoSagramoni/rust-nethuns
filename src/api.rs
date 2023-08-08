use crate::types::NethunsQueue;


/// Get full device name, taking into account 
/// both the real device name and the queue
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
