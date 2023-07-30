use derivative::Derivative;
use libc::timeval;
use std::cmp::Ordering;

#[derive(Clone, Debug, Derivative, PartialEq)]
#[derivative(Default, PartialOrd)]
#[repr(C)]
pub struct Pkthdr {
    #[derivative(Default(value = "timeval { tv_sec: 0, tv_usec: 0 }"))]
    #[derivative(PartialOrd(compare_with = "partial_ord_timeval"))]
    ts: timeval,
    len: u32,
    caplen: u32,
    buf_idx: u32,
}


/// Implement custom ordering of libc::timeval
fn partial_ord_timeval(t1: &timeval, t2: &timeval) -> Option<Ordering> {
    if t1.tv_sec < t2.tv_sec {
        return Some(Ordering::Less);
    } else if t1.tv_sec > t2.tv_sec {
        return Some(Ordering::Greater);
    } else if t1.tv_usec < t2.tv_usec {
        return Some(Ordering::Less);
    } else if t1.tv_usec > t2.tv_usec {
        return Some(Ordering::Greater);
    } else {
        return Some(Ordering::Equal);
    }
}
