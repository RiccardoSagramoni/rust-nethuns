use crate::types::NethunsQueue;

cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        mod netmap;
        pub use netmap::*;
    } else {
        std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
    }
}

pub mod errors;

#[inline(always)]
fn nethuns_dev_queue_name(dev: Option<&str>, queue: NethunsQueue) -> String {
    return match dev {
        None => "unspec".to_owned(),
        Some(dev) => match queue {
            NethunsQueue::Some(idx) => {
                format!("{}:{}", dev, idx)
            }
            NethunsQueue::Any => dev.to_owned(),
        },
    };
}
