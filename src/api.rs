use crate::types::NethunsQueue;


#[inline(always)]
pub fn nethuns_dev_queue_name(dev: Option<&str>, queue: NethunsQueue) -> String {
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
