//! Helper module for drop-safe `HybridRc<[T]>` construction

use super::{RcBox, RcMeta};
use core::mem::{forget, MaybeUninit};
use core::ptr::drop_in_place;

// Builder to create a new `RcBox<[T]>` of a given length
//
// In the event of a panic, elements that have been already appended will be correctly dropped.
#[must_use]
pub(super) struct SliceBuilder<'a, T> {
    rcbox: &'a mut RcBox<[MaybeUninit<T>]>,
    n_elems: usize,
}

impl<'a, T> SliceBuilder<'a, T> {
    /// Constructs a new builder for a `RcBox<[T]>` with a slice length of `length`
    #[inline]
    pub fn new(meta: RcMeta, length: usize) -> Self {
        let rcbox = RcBox::<T>::allocate_slice(meta, length, false);
        Self { rcbox, n_elems: 0 }
    }
    
    /// Fills the next free slot in the slice with `item`
    #[inline]
    pub fn append(&mut self, item: T) {
        self.rcbox.data[self.n_elems].write(item);
        self.n_elems += 1;
    }
    
    /// Consumes the builder and returns the initialized `RcBox<T>`
    ///
    /// The result is a mutable reference with arbitrary lifetime.
    ///
    /// # Panics
    /// Panics if the number of appended elements doesn't match the promised length.
    #[inline]
    pub fn finish(self) -> &'a mut RcBox<[T]> {
        assert_eq!(self.n_elems, self.rcbox.data.len());
        let rcbox: *mut _ = self.rcbox;
        forget(self);
        unsafe { (*rcbox).assume_init() }
    }
}

impl<T> Drop for SliceBuilder<'_, T> {
    /// Drops the already cloned elements and deallocates the temporary `RcBox`
    ///
    /// Only reached if the builder wasn't consumed by `finish`, which should only happen in
    /// a panic unwind.
    #[cold]
    fn drop(&'_ mut self) {
        let slice = &mut self.rcbox.data[..self.n_elems];
        unsafe {
            let slice: &mut [T] =
                &mut *(slice as *mut [MaybeUninit<T>] as *mut [T]);
            drop_in_place(slice);
        }
        unsafe {
            RcBox::dealloc(self.rcbox.into());
        }
    }
}
