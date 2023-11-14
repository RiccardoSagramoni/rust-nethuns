//! Helper module for thread identification

use core::convert::TryInto;
use core::num::NonZeroUsize;

thread_local! {
    /// Zero-sized thread-local variable to differentiate threads.
    static THREAD_MARKER: () = ();
}

const SENITEL: NonZeroUsize =
    unsafe { NonZeroUsize::new_unchecked(usize::MAX) };

/// A unique identifier for a running thread.
///
/// Uniqueness is guaranteed between running threads. However, the ids of dead
/// threads may be reused.
///
/// There is a chance that this implementation can be replaced by [`std::thread::ThreadId`]
/// when [`as_u64()`] is stabilized.
///
/// **Note:** The current (non platform specific) implementation uses the address of a
/// thread local static variable for thread identification.
///
/// [`as_u64()`]: std::thread::ThreadId::as_u64
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub(super) struct ThreadId(pub(super) NonZeroUsize);

impl ThreadId {
    /// Creates a new `ThreadId` for the given raw id.
    #[inline(always)]
    pub(super) const fn new(value: NonZeroUsize) -> Self {
        Self(value)
    }
    
    /// Gets the id for the thread that invokes it.
    #[inline]
    pub(super) fn current_thread() -> Self {
        Self::new(
            THREAD_MARKER
                .try_with(|x| x as *const _ as usize)
                .expect("the thread's local data has already been destroyed")
                .try_into()
                .expect("thread id should never be zero"),
        )
    }
}

impl PartialEq for ThreadId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (SENITEL, _) | (_, SENITEL) => false,
            (a, b) => a == b,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use core::convert::TryInto;
    use std::thread;
    
    /// Tests if the thread id stays the same on the same thread.
    #[test]
    fn test_thread_ids_eq() {
        let a = ThreadId::current_thread();
        let b = ThreadId::current_thread();
        assert_eq!(a, b);
        assert_eq!(format!("{:?}", &a), format!("{:?}", &b));
    }
    
    /// Tests if thread id that aren't the senitel compare as expected
    #[test]
    fn test_thread_ids_eq_non_senital() {
        let a = ThreadId::new(32.try_into().unwrap());
        let b = ThreadId::new(32.try_into().unwrap());
        let c = ThreadId::new(16.try_into().unwrap());
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(format!("{:?}", &a), format!("{:?}", &b));
    }
    
    /// Tests if the thread id of two different threads differ.
    #[test]
    fn test_thread_ids_ne() {
        let a = ThreadId::current_thread();
        let b = thread::spawn(move || ThreadId::current_thread())
            .join()
            .unwrap();
        assert_ne!(a, b);
        assert_ne!(format!("{:?}", &a), format!("{:?}", &b));
    }
    
    /// Tests if senitel thread ids compare unequal to anything
    #[test]
    fn test_thread_senitel_ne() {
        let a = ThreadId::new(SENITEL);
        let b = ThreadId::new(SENITEL);
        let c = ThreadId::new(32.try_into().unwrap());
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(c, b);
    }
    
    #[test]
    fn test_thread_ids_clone() {
        let a = ThreadId::current_thread();
        let b = a.clone();
        assert_eq!(a, b);
    }
    
    #[test]
    fn test_debug_strings() {
        let a = ThreadId::current_thread();
        let b = ThreadId::current_thread();
        let c = ThreadId::new(1.try_into().unwrap());
        
        assert_eq!(format!("{:?}", &a), format!("{:?}", &b));
        assert_ne!(format!("{:?}", &a), format!("{:?}", &c));
    }
}
