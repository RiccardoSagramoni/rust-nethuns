use derivative::Derivative;
use libc::timeval;
use std::cmp::Ordering;

#[repr(C)]
#[derive(Clone, Debug, Derivative, PartialEq)]
#[derivative(Default, PartialOrd)]
pub struct Pkthdr {
    #[derivative(Default(value = "timeval { tv_sec: 0, tv_usec: 0 }"))]
    #[derivative(PartialOrd(compare_with = "partial_ord_timeval"))]
    pub ts: timeval,
    pub len: u32,
    pub caplen: u32,
    pub buf_idx: u32,
}


/// Implement custom ordering of libc::timeval
fn partial_ord_timeval(t1: &timeval, t2: &timeval) -> Option<Ordering> {
    if t1.tv_sec < t2.tv_sec {
        Some(Ordering::Less)
    } else if t1.tv_sec > t2.tv_sec {
        Some(Ordering::Greater)
    } else if t1.tv_usec < t2.tv_usec {
        Some(Ordering::Less)
    } else if t1.tv_usec > t2.tv_usec {
        Some(Ordering::Greater)
    } else {
        Some(Ordering::Equal)
    }
}
