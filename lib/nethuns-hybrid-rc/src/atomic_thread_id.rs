//! Helper module for atomically stored thread identification

#![forbid(unsafe_code)]

use core::convert::TryInto;
use core::sync::atomic::{AtomicUsize, Ordering};

pub(crate) use super::thread_id::ThreadId;

/// An [`Option`]`<`[`ThreadId`]`>` which can be safely shared between threads.
///
/// **Note:** Currently implemented as a wrapper around [`AtomicUsize`].
#[derive(Debug)]
#[repr(transparent)]
pub(crate) struct AtomicOptionThreadId(core::sync::atomic::AtomicUsize);

/// Converts the internal representation in [`AtomicOptionThreadId`] into
/// `Some(ThreadId)` or `None`.
///
/// This should only be a type-level conversion and a noop at runtime.
#[inline(always)]
fn wrap(value: usize) -> Option<ThreadId> {
	match value {
		0 => None,
		n => Some(ThreadId::new(n.try_into().unwrap())),
	}
}

/// Converts an `Option<ThreadId>` into the internal representation for
/// [`AtomicOptionThreadId`].
///
/// This should only be a type-level conversion and a noop at runtime.
#[inline(always)]
const fn unwrap(value: Option<ThreadId>) -> usize {
	match value {
		None => 0,
		Some(id) => id.0.get(),
	}
}

impl AtomicOptionThreadId {
	/// Creates a new `AtomicOptionThreadId`.
	#[inline]
	pub const fn new(id: Option<ThreadId>) -> Self {
		Self(AtomicUsize::new(unwrap(id)))
	}

	/// Loads a value from the atomic.
	///
	/// The [`Ordering`] may only be `SeqCst`, `Acquire` or `Relaxed`.
	///
	/// # Panics
	///
	/// Panics if `order` is `Release` or `AcqRel`.
	#[inline]
	pub fn load(&self, order: Ordering) -> Option<ThreadId> {
		wrap(self.0.load(order))
	}

	/// Stores a value into the atomic.
	///
	/// The [`Ordering`] may only be `SeqCst`, `Release` or `Relaxed`.
	///
	/// # Panics
	///
	/// Panics if `order` is `Acquire` or `AcqRel`.
	#[inline]
	pub fn store(&self, val: Option<ThreadId>, order: Ordering) {
		self.0.store(unwrap(val), order);
	}

	/// Stores `new` into the atomic iff the currently stored value is `None`.
	///
	/// The success ordering may be any [`Ordering`], but `failure` may only be
	/// `SeqCst`, `Release` or `Relaxed` and must be equivalent to or weaker than
	/// `success`.
	///
	/// # Panics
	///
	/// Panics if `failure` is `Acquire`, `AcqRel` or a stronger ordering than
	/// `success`.
	#[inline]
	pub fn store_if_none(
		&self,
		new: Option<ThreadId>,
		success: Ordering,
		failure: Ordering,
	) -> Result<Option<ThreadId>, Option<ThreadId>> {
		self.0
			.compare_exchange(unwrap(None), unwrap(new), success, failure)
			.map(wrap)
			.map_err(wrap)
	}
}

impl Default for AtomicOptionThreadId {
	/// Creates an `AtomicOptionThreadId` initialized to [`None`].
	#[inline]
	fn default() -> Self {
		Self::new(None)
	}
}

impl From<ThreadId> for AtomicOptionThreadId {
	/// Converts a [`ThreadId`] into an `AtomicOptionThreadId`, wrapping it in [`Some`].
	#[inline]
	fn from(id: ThreadId) -> Self {
		Self::new(Some(id))
	}
}

impl From<Option<ThreadId>> for AtomicOptionThreadId {
	/// Converts an [`Option`]`<`[`ThreadId`]`>` into an `AtomicOptionThreadId`.
	#[inline]
	fn from(id: Option<ThreadId>) -> Self {
		Self::new(id)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use alloc::format;

	/// Tests if the thread id of two different threads differ.
	#[test]
	fn test_wrapping() {
		let a = ThreadId::current_thread();

		assert_ne!(None, wrap(unwrap(Some(a))));
		assert_eq!(None, wrap(unwrap(None)));
	}

	/// Tests if the thread id of two different threads differ.
	#[test]
	fn test_atomic() {
		let tid = ThreadId::current_thread();
		let a: AtomicOptionThreadId = tid.into();
		let b = AtomicOptionThreadId::default();
		let _ = AtomicOptionThreadId::new(None);

		assert_ne!(a.load(Ordering::Relaxed), None);
		assert_eq!(b.load(Ordering::Relaxed), None);

		a.store(None, Ordering::Relaxed);
		b.store(Some(tid), Ordering::Relaxed);

		assert_eq!(a.load(Ordering::Relaxed), None);
		assert_ne!(b.load(Ordering::Relaxed), None);

		assert!(a
			.store_if_none(Some(tid), Ordering::Relaxed, Ordering::Relaxed)
			.is_ok());
		assert!(a
			.store_if_none(None, Ordering::Relaxed, Ordering::Relaxed)
			.is_err());

		assert_ne!(a.load(Ordering::Relaxed), None);
		assert_ne!(b.load(Ordering::Relaxed), None);
		assert_eq!(format!("{:?}", &a), format!("{:?}", &b));
	}
}
