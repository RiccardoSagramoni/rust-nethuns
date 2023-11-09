/*!
 * Thread-safe hybrid reference counting pointers
 *
 * Loosely based on the algorithm described in
 * ["Biased reference counting: minimizing atomic operations in garbage collection"][doi:10.1145/3243176.3243195]
 * by Jiho Choi et. al. but adapted to Rust's type system and its lack of a managed runtime
 * environment.
 *
 * The type `HybridRc<T, State>` provides thread-safe shared ownership of a value of type `T`
 * allocated on the heap, just like `std::sync::Arc<T>` does. The main difference is that one
 * thread at a time can use non-atomic reference counting for better performance. That means that
 * `HybridRc` is especially suited for workloads where one thread accesses the shared value
 * significantly more often than others.
 *
 * There a two variants of [`HybridRc`]:
 * - `HybridRc<T, `[`Local`]`>` (type aliased as [`Rc`]): very fast but only usable on one thread.
 * - `HybridRc<T, `[`Shared`]`>` (type aliased as [`Arc`]): slower but universally usable.
 *
 * Instances of both variants are convertible into each other. Especially, an `Rc` can always be
 * converted into an `Arc` using [`HybridRc::to_shared(&rc)`] or [`.into()`].
 *
 * An `Arc` on the other hand can only be converted into an `Rc` using [`HybridRc::to_local(&arc)`]
 * or [`.try_into()`] if no other thread has `Rc`s for the same value. The thread holding `Rc`s to
 * a value is called the "owner thread". Once all `Rc`s are dropped, the shared value becomes
 * ownerless again.
 *
 * `HybridRc` is designed as a drop-in replacement for `std::sync::Arc` and `std::rc::Rc`, so except
 * for the conversion functionality outlined above the usage is similar to these and other smart
 * pointers.
 *
 * # Thread Safety
 *
 * `HybridRc` uses two separate reference counters - one modified non-atomically and one using
 * atomic operations - and keeps track of a owner thread that is allowed to modify the "local"
 * reference counter. This means that it is thread-safe, while one thread is exempted from
 * the disadvantage of atomic operations being more expensive than ordinary memory accesses.
 *
 * # Examples
 *
 * Multiple threads need a reference to a shared value while one thread needs to clone references
 * to the value significantly more often than the others.
 * ```ignore
 * use hybrid_rc::{Rc, Arc};
 * use std::thread;
 * use std::sync::mpsc::channel;
 *
 * # type SomeComplexType = std::collections::BinaryHeap<()>;
 * # fn expensive_computation<T>(x: impl AsRef<T>, i: i32) -> i32 { let _ = x.as_ref(); i }
 * # fn do_something<T>(x: impl AsRef<T>, _i: i32) { let _ = x.as_ref(); }
 * # fn main() -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
 * let local = Rc::new(SomeComplexType::new());
 * let (sender, receiver) = channel();
 *
 * // Spawn of threads for multiple expensive computations
 * for i in 1..=4 {
 *     let sender = sender.clone();
 *     let shared = Rc::to_shared(&local);
 *     thread::spawn(move || {
 *         sender.send(expensive_computation(shared, i));
 *     });
 * }
 *
 * // Do something that needs single-thread reference counting
 * for i in 1..=1000 {
 *     do_something(local.clone(), i);
 * }
 *
 * // Collect expensive computation results
 * for i in 1..=4 {
 *     println!("{:?}", receiver.recv().unwrap());
 * }
 * # Ok(())
 * # }
 * ```
 *
 * A library wants to give library consumers flexibility for multithreading but also internally
 * have the performance of `std::rc::Rc` for e.g. a complex tree structure that is mutated on
 * the main thread.
 * ```ignore
 * use hybrid_rc::Rc;
 * use std::thread;
 *
 * # fn get_local_hybridrc_from_some_library() -> Rc<()> { Rc::default() }
 * # fn do_something(_: &()) { }
 * # fn main() -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
 * let reference = get_local_hybridrc_from_some_library();
 * let shared = Rc::to_shared(&reference);
 *
 * // do the work in another thread
 * let worker = thread::spawn(move || {
 *     do_something(&*shared);
 * });
 *
 * // Do something useful with the library
 *
 * worker.join()?;
 * # Ok(())
 * # }
 * ```
 *
 * [`HybridRc::to_shared(&rc)`]: HybridRc::to_shared
 * [`HybridRc::to_local(&arc)`]: HybridRc::to_local
 * [`.into()`]: HybridRc#impl-From<HybridRc<T%2C%20Local>>
 * [`.try_into()`]: HybridRc#impl-TryFrom<HybridRc<T%2C%20Shared>>
 * [doi:10.1145/3243176.3243195]: https://dl.acm.org/doi/10.1145/3243176.3243195
 */

#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;
use alloc::alloc::Layout;
use alloc::borrow::{Cow, ToOwned};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use std::any::Any;
use std::borrow::Borrow;
use std::cell::Cell;
use std::convert::{Infallible, TryFrom};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic;
use std::sync::atomic::Ordering;
use std::{cmp, fmt, iter, mem, ptr};

mod atomic_thread_id;
use atomic_thread_id::{AtomicOptionThreadId, ThreadId};
mod slice_builder;
use slice_builder::SliceBuilder;
mod tests;
mod thread_id;

/// Provides a senitel pointer value for dangling `Weak`s.
///
/// This is not NULL to allow optimizations through [`NonNull`] but cannot ever be a valid pointer
/// to a [`RcBox`].
#[inline]
const fn senitel<T>() -> NonNull<T> {
    unsafe { NonNull::new_unchecked(usize::MAX as *mut T) }
}

/// Checks if the provided pointer is the [`senitel`]
#[inline]
fn is_senitel<T: ?Sized>(ptr: *const T) -> bool {
    ptr.cast::<()>() == senitel().as_ptr()
}

/// Module for definition of `RcState`.
pub mod state_trait {
    use core::fmt::Debug;
    
    /// Internal trait for type-level enumeration of `Shared` and `Local`.
    pub trait RcState: Debug {
        const SHARED: bool;
    }
}
use state_trait::RcState;

/// Marker types for the states of a [`HybridRc`]
pub mod state {
    /// Marks a [`HybridRc`] as shared.
    ///
    /// `HybridRc<_, Shared>` atomically updates the shared reference counter.
    ///
    /// # See also
    /// - [`Local`]
    ///
    /// [`HybridRc`]: super::HybridRc
    #[derive(Debug, Clone, Copy)]
    pub enum Shared {}
    impl super::RcState for Shared {
        const SHARED: bool = true;
    }
    
    /// Marks a [`HybridRc`] as local.
    ///
    /// `HybridRc<_, Local>` non-atomically updates the local reference counter.
    ///
    /// # See also
    /// - [`Shared`]
    ///
    /// [`HybridRc`]: super::HybridRc
    #[derive(Debug, Clone, Copy)]
    pub enum Local {}
    impl super::RcState for Local {
        const SHARED: bool = false;
    }
}
use state::{Local, Shared};

/// An enumeration of possible errors when upgrading a [`Weak`].
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UpgradeError {
    /// The referenced value was already dropped because no strong references to it exists anymore.
    ValueDropped,
    /// The requested action would have created a new [`Rc`] while at least one `Rc` still existed
    /// on another thread.
    WrongThread,
}

impl fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ValueDropped => f.write_str("value was already dropped"),
            Self::WrongThread => {
                f.write_str("tried to get a local reference while another thread was the owner")
            }
        }
    }
}

impl std::error::Error for UpgradeError {}

impl From<Infallible> for UpgradeError {
    fn from(x: Infallible) -> UpgradeError {
        match x {}
    }
}

/// The `AllocError` error indicates an allocation failure when using `try_new()` etc.
///
/// Will become a type alias for [`std::alloc::AllocError`] once that is stabilized.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct AllocError;

impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("memory allocation failed")
    }
}

impl std::error::Error for AllocError {}

impl From<Infallible> for AllocError {
    fn from(_: Infallible) -> AllocError {
        unreachable!();
    }
}

/// Reimplementation of `ptr::set_ptr_value` as long as that one is unstable
///
/// Constructs a new pointer to `addr_ptr` with the metadata and type of `meta_ptr`.
#[inline]
fn set_ptr_value<T: ?Sized, U>(
    mut meta_ptr: *const T,
    addr_ptr: *mut U,
) -> *mut T {
    let thin = (&mut meta_ptr as *mut *const T).cast::<*const u8>();
    // Safety: In case of a thin pointer, this operations is identical
    // to a simple assignment. In case of a fat pointer, with the current
    // fat pointer layout implementation, the first field of such a
    // pointer is always the data pointer, which is likewise assigned.
    unsafe { *thin = addr_ptr.cast() };
    
    meta_ptr as *mut T
}

/// Metadata part of a shared allocation.
struct RcMeta {
    /// Id for the thread which may use local references
    owner: AtomicOptionThreadId,
    /// Strong local reference count
    strong_local: Cell<usize>,
    /// Strong shared reference count (+ 1 for all strong local references combined)
    strong_shared: atomic::AtomicUsize,
    
    /// Weak reference count (+ 1 for all strong references combined)
    ///
    /// If `usize::MAX`, the ability to downgrade strong pointers is temporarily locked to avoid
    /// races in `get_mut()`.
    weak: atomic::AtomicUsize,
}

/// Heap struct for shared allocations of `T`.
///
/// `repr(C)` to future-proof against possible layout optimizations which could interfere with
/// `[into|from]_raw()` of transmutable data types.
#[repr(C)]
struct RcBox<T: ?Sized> {
    meta: RcMeta,
    data: T,
}

impl<T: ?Sized> RcBox<T> {
    /// Deallocates an `RcBox`
    ///
    /// `meta` will be dropped, but `data` must have already been dropped in place.
    ///
    /// # Safety
    /// The allocation must have been previously allocated with `RcBox::allocate_*()`.
    #[inline]
    unsafe fn dealloc(ptr: NonNull<RcBox<T>>) {
        unsafe { ptr::addr_of_mut!((*ptr.as_ptr()).meta).drop_in_place() };
        let layout = Layout::for_value(unsafe { ptr.as_ref() });
        unsafe { alloc::alloc::dealloc(ptr.as_ptr().cast(), layout) };
    }
    
    /// Tries to allocate an `RcBox` for a possibly dynamically sized value
    ///
    /// Size and alignment of `example` are used for allocation and if `example` is a fat reference
    /// the pointer metadata is copied to the resulting pointer.
    ///
    /// Returns a mutable pointer on success and the memory layout that could not be allocated
    /// if the allocation failed.
    #[inline]
    fn try_allocate_for_val(
        meta: RcMeta,
        example: &T,
        zeroed: bool,
    ) -> Result<NonNull<RcBox<T>>, Layout> {
        let layout = Layout::new::<RcBox<()>>();
        let layout = layout
            .extend(Layout::for_value(example))
            .map_err(|_| layout)?
            .0
            .pad_to_align();
        
        // Allocate memory
        let ptr = unsafe {
            if zeroed {
                alloc::alloc::alloc_zeroed(layout)
            } else {
                alloc::alloc::alloc(layout)
            }
        }
        .cast::<RcBox<()>>();
        
        // Write RcMeta fields
        // Safety: Freshly allocated, so valid to write to.
        unsafe { ptr::addr_of_mut!((*ptr).meta).write(meta) };
        
        // Combine metadata from `example` with new memory
        let result = set_ptr_value(example, ptr);
        
        NonNull::new(result as *mut RcBox<T>).ok_or(layout)
    }
    
    /// Allocates an `RcBox` for a possibly dynamically sized value
    ///
    /// Size and alignment of `example` are used for allocation and if `example` is a fat reference
    /// the pointer metadata is copied to the resulting pointer.
    ///
    /// Returns a mutable pointer on success.
    ///
    /// # Panics
    /// Panics or aborts if the allocation failed.
    #[inline]
    fn allocate_for_val(
        meta: RcMeta,
        example: &T,
        zeroed: bool,
    ) -> NonNull<RcBox<T>> {
        match Self::try_allocate_for_val(meta, example, zeroed) {
            Ok(result) => result,
            Err(layout) => alloc::alloc::handle_alloc_error(layout),
        }
    }
    
    /// Get the pointer to a `RcBox<T>` from a pointer to the data
    ///
    /// # Safety
    ///
    /// The pointer must point to (and have valid metadata for) the data part of a previously
    /// valid instance of `RcBox<T>` and it must not be dangling.
    #[inline]
    unsafe fn ptr_from_data_ptr(ptr: *const T) -> *const RcBox<T> {
        // Calculate layout of RcBox<T> without `data` tail, but including padding
        let base_layout = Layout::new::<RcBox<()>>();
        // Safety: covered by the safety contract above
        let value_alignment = mem::align_of_val(unsafe { &*ptr });
        let value_offset_layout = Layout::from_size_align(0, value_alignment)
            .expect("invalid memory layout");
        let layout = base_layout
            .extend(value_offset_layout)
            .expect("invalid memory layout")
            .0;
        
        // Move pointer to point to the start of the original RcBox<T>
        // Safety: covered by the safety contract above
        let rcbox =
            unsafe { ptr.cast::<u8>().offset(-(layout.size() as isize)) };
        set_ptr_value(ptr, rcbox as *mut u8) as *const RcBox<T>
    }
}

impl<T> RcBox<T> {
    /// Tries to allocate an `RcBox`
    ///
    /// Returns a mutable reference with arbitrary lifetime on success and the memory layout that
    /// could not be allocated if the allocation failed.
    #[inline]
    fn try_allocate(
        meta: RcMeta,
    ) -> Result<NonNull<RcBox<mem::MaybeUninit<T>>>, Layout> {
        let layout = Layout::new::<RcBox<T>>();
        
        let ptr = unsafe { alloc::alloc::alloc(layout) }
            .cast::<RcBox<mem::MaybeUninit<T>>>();
        if ptr.is_null() {
            Err(layout)
        } else {
            unsafe { ptr::addr_of_mut!((*ptr).meta).write(meta) };
            Ok(unsafe { NonNull::new_unchecked(ptr) })
        }
    }
    
    /// Allocates an `RcBox`
    ///
    /// Returns a mutable reference with arbitrary lifetime on success.
    ///
    /// # Panics
    /// Panics or aborts if the allocation failed.
    #[inline]
    fn allocate(meta: RcMeta) -> NonNull<RcBox<mem::MaybeUninit<T>>> {
        match Self::try_allocate(meta) {
            Ok(result) => result,
            Err(layout) => alloc::alloc::handle_alloc_error(layout),
        }
    }
    
    /// Tries to allocate an `RcBox` for a slice.
    ///
    /// Returns a mutable reference with arbitrary lifetime on success and the memory layout that
    /// could not be allocated if the allocation failed or the layout calculation overflowed.
    #[inline]
    fn try_allocate_slice<'a>(
        meta: RcMeta,
        len: usize,
        zeroed: bool,
    ) -> Result<&'a mut RcBox<[mem::MaybeUninit<T>]>, Layout> {
        // Calculate memory layout
        let layout = Layout::new::<RcBox<[T; 0]>>();
        let payload_layout = Layout::array::<T>(len).map_err(|_| layout)?;
        let layout = layout
            .extend(payload_layout)
            .map_err(|_| layout)?
            .0
            .pad_to_align();
        
        // Allocate memory
        let ptr = unsafe {
            if zeroed {
                alloc::alloc::alloc_zeroed(layout)
            } else {
                alloc::alloc::alloc(layout)
            }
        };
        
        // Build a fat pointer
        // The immediate slice reference [MaybeUninit<u8>] *should* be sound
        let ptr = ptr::slice_from_raw_parts_mut(
            ptr.cast::<mem::MaybeUninit<u8>>(),
            len,
        ) as *mut RcBox<[mem::MaybeUninit<T>]>;
        
        if ptr.is_null() {
            // Allocation failed
            Err(layout)
        } else {
            // Initialize metadata field and return result
            unsafe { ptr::addr_of_mut!((*ptr).meta).write(meta) };
            Ok(unsafe { ptr.as_mut().unwrap() })
        }
    }
    
    /// Allocates an `RcBox` for a slice
    ///
    /// Returns a mutable reference with arbitrary lifetime on success.
    ///
    /// # Panics
    /// Panics or aborts if the allocation failed or the memory layout calculation overflowed.
    #[inline]
    fn allocate_slice<'a>(
        meta: RcMeta,
        len: usize,
        zeroed: bool,
    ) -> &'a mut RcBox<[mem::MaybeUninit<T>]> {
        match Self::try_allocate_slice(meta, len, zeroed) {
            Ok(result) => result,
            Err(layout) => alloc::alloc::handle_alloc_error(layout),
        }
    }
}

impl<T> RcBox<mem::MaybeUninit<T>> {
    /// Converts to a mutable reference without the `MaybeUninit` wrapper.
    ///
    /// # Safety
    /// The payload must have been fully initialized or this causes immediate undefined behaviour.
    #[inline]
    unsafe fn assume_init(&mut self) -> &mut RcBox<T> {
        unsafe { (self as *mut Self).cast::<RcBox<T>>().as_mut() }.unwrap()
    }
}

impl<T> RcBox<[mem::MaybeUninit<T>]> {
    /// Converts to a mutable reference without the `MaybeUninit` wrapper.
    ///
    /// # Safety
    /// The payload slice must have been fully initialized or this causes immediate undefined
    /// behaviour.
    #[inline]
    unsafe fn assume_init(&mut self) -> &mut RcBox<[T]> {
        unsafe { (self as *mut _ as *mut RcBox<[T]>).as_mut() }.unwrap()
    }
}

impl RcMeta {
    /// Increments the local reference counter unconditionally.
    ///
    /// *Only safe to use on the owner thread and as long as at least one local reference exists.*
    ///
    /// # Panics
    /// Panics if the counter overflowed.
    #[inline(always)]
    fn inc_strong_local(&self) {
        let counter = self.strong_local.get();
        
        if counter == usize::MAX {
            panic!("reference counter overflow");
        }
        
        self.strong_local.set(counter + 1);
    }
    
    /// Increment the local reference counter.
    ///
    /// Also adjusts the shared reference counter if neccessary.
    ///
    /// Fails if this would resurrect an already dropped
    /// value.
    ///
    /// *Only safe to use on the owner thread.*
    ///
    /// # Panics
    /// Panics if one of the counters overflowed.
    #[inline]
    fn try_inc_strong_local(&self) -> Result<(), ()> {
        let counter = self.strong_local.get();
        
        if counter == usize::MAX {
            panic!("reference counter overflow");
        } else if counter == 0 {
            self.try_inc_strong_shared()?;
        }
        
        self.strong_local.set(counter + 1);
        Ok(())
    }
    
    /// Decrements the local reference counter.
    ///
    /// Also adjusts the shared reference counter and
    /// the `owner` if neccessary.
    ///
    /// Returns **true** if no strong references remain at all.
    ///
    /// *Only safe to use on the owner thread.*
    ///
    /// # Panics
    /// Panics if the shared reference counter was already zero.
    #[inline(always)]
    fn dec_strong_local(&self) -> bool {
        let counter = self.strong_local.get();
        self.strong_local.set(counter - 1);
        if counter == 1 {
            self.remove_last_local_reference()
        } else {
            false
        }
    }
    
    /// Decrements the shared counter and sets the `owner` to `None`.
    ///
    /// Used internally by `dec_strong_local()`
    ///
    /// # Panics
    /// Panics if the counter was already zero.
    fn remove_last_local_reference(&self) -> bool {
        let old_shared = self.strong_shared.fetch_sub(1, Ordering::Release);
        if old_shared == 0 {
            panic!("reference counter underflow");
        }
        self.owner.store(None, Ordering::Release);
        old_shared == 1
    }
    
    /// Increments the shared reference counter unconditionally.
    ///
    /// *Only safe to use as long as at least one shared reference exists.*
    ///
    /// # Panics
    /// Panics if the counter overflowed.
    #[inline]
    fn inc_strong_shared(&self) {
        let old_counter = self.strong_shared.fetch_add(1, Ordering::Relaxed);
        if old_counter == usize::MAX {
            panic!("reference counter overflow");
        }
    }
    
    /// Increments the shared reference counter.
    ///
    /// Also adjusts the shared reference counter and the `owner` if neccessary.
    ///
    /// Fails if this would resurrect an already dropped value.
    ///
    /// # Panics
    /// Panics if the counter overflowed.
    #[inline]
    fn try_inc_strong_shared(&self) -> Result<(), ()> {
        self.strong_shared
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old_counter| {
                match old_counter {
                    0 => None,
                    usize::MAX => panic!("reference counter overflow"),
                    _ => Some(old_counter + 1),
                }
            })
            .map(|_| ())
            .map_err(|_| ())
    }
    
    /// Decrements the shared reference counter.
    ///
    /// Returns **true** if no strong references remain at all.
    ///
    /// # Panics
    /// Panics if the counter was already zero.
    #[inline]
    fn dec_strong_shared(&self) -> bool {
        let old_counter = self.strong_shared.fetch_sub(1, Ordering::Release);
        if old_counter == 0 {
            panic!("reference counter underflow");
        }
        old_counter == 1 && self.owner.load(Ordering::Relaxed).is_none()
    }
    
    /// Increments the weak reference counter.
    ///
    /// # Panics
    /// Panics if the counter overflowed or was already zero.
    #[inline]
    fn inc_weak(&self) {
        const MAX_COUNT: usize = usize::MAX - 1;
        let mut counter = self.weak.load(Ordering::Relaxed);
        
        // CAS loop
        loop {
            match counter {
                usize::MAX => {
                    core::hint::spin_loop();
                    counter = self.weak.load(Ordering::Relaxed);
                    continue;
                }
                MAX_COUNT => panic!("weak counter overflow"),
                0 => panic!("BUG: weak resurrection of dead counted reference"),
                _ => {
                    let result = self.weak.compare_exchange_weak(
                        counter,
                        counter + 1,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    );
                    match result {
                        Ok(_) => break,
                        Err(old) => counter = old,
                    }
                }
            }
        }
    }
    
    /// Increments the weak reference counter (without a spin loop).
    ///
    /// # Panics
    /// Panics if the counter is locked, overflowed or was already zero.
    #[inline]
    fn inc_weak_nolock(&self) {
        const MAX_COUNT: usize = usize::MAX - 1;
        match self.weak.fetch_add(1, Ordering::Relaxed) {
            usize::MAX => panic!("BUG: weak counter locked"),
            MAX_COUNT => panic!("weak counter overflow"),
            0 => panic!("BUG: weak resurrection of dead counted reference"),
            _ => (),
        }
    }
    
    /// Decrements the weak reference counter.
    ///
    /// Returns **true** if the counter reached zero.
    ///
    /// # Panics
    /// Panics if the counter was already zero.
    #[inline]
    fn dec_weak(&self) -> bool {
        let old_counter = self.weak.fetch_sub(1, Ordering::Release);
        if old_counter == 0 {
            panic!("weak counter underflow");
        }
        old_counter == 1
    }
    
    /// Checks if there is only one unique reference.
    ///
    /// If `is_local` is true, it is assumed that we can access the local reference counter
    ///
    /// Temporarily locks the weak reference counter to prevent race conditions.
    #[inline]
    fn has_unique_ref(&self, is_local: bool) -> bool {
        let result = self.weak.compare_exchange(
            1,
            usize::MAX,
            Ordering::Acquire,
            Ordering::Relaxed,
        );
        if result.is_ok() {
            let mut count = self.strong_shared.load(Ordering::Acquire);
            
            if count == 1 {
                let owner = self.owner.load(Ordering::Relaxed);
                match owner {
                    None => {}
                    Some(tid)
                        if is_local || tid == ThreadId::current_thread() =>
                    {
                        count = self.strong_local.get();
                    }
                    Some(_) => {
                        count = 2;
                    }
                }
            }
            
            self.weak.store(1, Ordering::Release);
            
            count == 1
        } else {
            false
        }
    }
}

/// A hybrid reference-counting pointer.
///
/// - [`HybridRc<T, Shared>`][Arc] behaves mostly like [`std::sync::Arc`]
/// - [`HybridRc<T, Local>`][Rc] behaves mostly like [`std::rc::Rc`].
///
/// See the [module-level documentation][crate] for more details.
///
/// The inherent methods of `HybridRc` are all associated functions, which means that you have to
/// call them as e.g. [`HybridRc::get_mut(&mut x)`] instead of `x.get_mut()`. This avoids conflicts
/// with methods of the inner type `T`.
///
/// [`HybridRc::get_mut(&mut x)`]: Self::get_mut
#[must_use]
pub struct HybridRc<T: ?Sized, State: RcState> {
    ptr: NonNull<RcBox<T>>,
    phantom: PhantomData<State>,
    phantom2: PhantomData<RcBox<T>>,
}

/// Type alias for a local reference counting pointer.
///
/// Provided to ease migrating from [`std::rc::Rc`].
///
/// See the [module-level documentation][crate] for more details.
///
/// The inherent methods of `Rc` are all associated functions, which means that you have to call
/// them as e.g. [`Rc::to_shared(&x)`] instead of `x.to_shared()`. This avoids conflicts with
/// methods of the inner type `T`.
///
/// [`Rc::to_shared(&x)`]: Self::to_shared
pub type Rc<T> = HybridRc<T, Local>;

/// Type alias for a shared reference counting pointer.
///
/// Provided to ease migrating from [`std::sync::Arc`].
///
/// See the [module-level documentation] for more details.
///
/// The inherent methods of `Arc` are all associated functions, which means that you have to call
/// them as e.g. [`Arc::to_local(&x)`] instead of `x.to_local()`. This avoids conflicts with
/// methods of the inner type `T`.
///
/// [`Arc::to_local(&x)`]: Self::to_local
/// [module-level documentation]: crate
pub type Arc<T> = HybridRc<T, Shared>;

impl<T: ?Sized, State: RcState> HybridRc<T, State> {
    /// Creates a new `HybridRc` from a pointer to a shared allocation.
    ///
    /// The reference counters must have been updated by the caller.
    #[inline(always)]
    fn from_inner(ptr: NonNull<RcBox<T>>) -> Self {
        Self {
            ptr,
            phantom: PhantomData,
            phantom2: PhantomData,
        }
    }
    
    /// Provides a reference to the inner value.
    #[inline(always)]
    fn data(&self) -> &T {
        // Safety: as long as one HybridRc or Weak for this item exists, the memory stays allocated.
        unsafe { &(*self.ptr.as_ptr()).data }
    }
    
    /// Provides a reference to the shared metadata.
    #[inline(always)]
    fn meta(&self) -> &RcMeta {
        // Safety: as long as one HybridRc or Weak for this item exists, the memory stays allocated.
        unsafe { &(*self.ptr.as_ptr()).meta }
    }
    
    /// Provides a reference to the inner `HybridRc` of a `Pin<HybridRc<T>>`
    ///
    /// # Safety
    /// The caller must ensure that the reference is not used to move the value out of self.
    #[inline(always)]
    unsafe fn pin_get_ref(this: &Pin<Self>) -> &Self {
        // SAFETY: Pin is repr(transparent) and by contract the caller doesn't use the reference
        // to move the value.
        unsafe { &*(this as *const Pin<Self>).cast::<Self>() }
    }
    
    /// Returns a mutable reference to the value, without checking for uniqueness.
    ///
    /// # See also
    /// - [`get_mut()`], which is safe.
    ///
    /// # Safety
    /// No other `HybridRc` or [`Weak`] for the same value must be dereferenced for the duration of
    /// the returned borrow.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let mut a = Rc::new([1, 2, 3]);
    /// // We know that there can't be any other references yet, so getting a mutable reference
    /// // is safe here:
    /// let mut_ref = unsafe { Rc::get_mut_unchecked(&mut a) };
    /// mut_ref[0] = 42;
    ///
    /// assert_eq!(a[..], [42, 2, 3]);
    /// ```
    /// [`get_mut()`]: Self::get_mut
    #[must_use]
    #[inline]
    pub unsafe fn get_mut_unchecked(this: &mut Self) -> &mut T {
        unsafe { &mut (*this.ptr.as_ptr()).data }
    }
    
    /// Returns a mutable reference to the value, iff the value is not shared
    /// with another `HybridRc` or [`Weak`].
    ///
    /// Returns `None` otherwise.
    #[must_use]
    #[inline]
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        if this.meta().has_unique_ref(!State::SHARED) {
            unsafe { Some(Self::get_mut_unchecked(this)) }
        } else {
            None
        }
    }
    
    /// Provides a raw pointer to the referenced value
    ///
    /// The counts are not affected in any way and the `HybridRc` is not consumed.
    /// The pointer is valid for as long there exists at least one `HybridRc` for the value.
    #[must_use]
    #[inline]
    pub fn as_ptr(this: &Self) -> *const T {
        let ptr = this.ptr.as_ptr();
        
        // Safety: Neccessary for `from_raw()` (when implemented), retains provenance.
        // Besides that, does basically the same thing as `data()` or `get_mut_unchecked()`.
        unsafe { ptr::addr_of_mut!((*ptr).data) }
    }
    
    /// Consumes the `HybridRc<T, State>`, returning the wrapped pointer.
    ///
    /// To avoid a memory leak the pointer must be converted back to a `HybridRc` using
    /// [`HybridRc<T, State>::from_raw()`].
    #[must_use = "Memory will leak if the result is not used"]
    pub fn into_raw(this: Self) -> *const T {
        let ptr = Self::as_ptr(&this);
        mem::forget(this);
        ptr
    }
    
    /// Reconstructs a `HybridRc<T, State>` from a raw pointer.
    ///
    /// Creates a `HybridRc<T, State>` from a pointer that has been previously returned by
    /// a call to [`into_raw()`].
    ///
    /// # Safety
    ///
    /// The raw pointer must have been previously returned by a call to
    /// [`HybridRc<T, State>`][`into_raw()`] for the same `State` *and* the same `T` or another
    /// compatible type that has the same size and alignment. The latter case amounts to
    /// [`mem::transmute()`] and is likely to produce undefined behaviour if not handled correctly.
    ///
    /// The value must not have been dropped yet.
    ///
    /// [`into_raw()`]: Self::into_raw
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        // Safety: covered by the safety contract for this function
        let box_ptr = unsafe { RcBox::<T>::ptr_from_data_ptr(ptr) };
        
        Self::from_inner(
            NonNull::new(box_ptr as *mut _).expect("invalid pointer"),
        )
    }
    
    /// Creates a new [`Weak`] for the referenced value.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::{Rc, Weak};
    ///
    /// let strong = Rc::new(42i32);
    /// let weak = Rc::downgrade(&strong);
    ///
    /// assert_eq!(Rc::as_ptr(&strong), Weak::as_ptr(&weak));
    /// ```
    #[inline]
    pub fn downgrade(this: &Self) -> Weak<T> {
        this.meta().inc_weak();
        Weak { ptr: this.ptr }
    }
    
    /// Creates a new [`PinWeak`] for the referenced value.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::{Rc, Weak};
    ///
    /// let strong = Rc::pin(42i32);
    /// let weak = Rc::downgrade_pin(&strong);
    /// ```
    #[inline]
    pub fn downgrade_pin(this: &Pin<Self>) -> PinWeak<T> {
        // Safety: We are not moving anything and we don't expose a non-pinned pointer.
        let this = unsafe { Self::pin_get_ref(this) };
        PinWeak(Self::downgrade(this))
    }
    
    /// Checks if two `HybridRc`s point to the same allocation.
    #[inline]
    pub fn ptr_eq<S: RcState>(this: &Self, other: &HybridRc<T, S>) -> bool {
        this.ptr.as_ptr() == other.ptr.as_ptr()
    }
    
    /// Checks if two pinned `HybridRc`s point to the same allocation.
    #[inline]
    pub fn ptr_eq_pin<S: RcState>(
        this: &Pin<Self>,
        other: &Pin<HybridRc<T, S>>,
    ) -> bool {
        // SAFETY: we are not moving anything and we don't expose any pointers.
        let this = unsafe { Self::pin_get_ref(this) };
        let other = unsafe { HybridRc::<T, S>::pin_get_ref(other) };
        this.ptr.as_ptr() == other.ptr.as_ptr()
    }
    
    /// Gets the approximate number of strong pointers to the inner value.
    ///
    /// As shared pointers cannot access the local reference counter, `Arc::strong_count()` only
    /// provides a lower bound on the reference count at the moment of the call.
    ///
    /// Please also understand that, if the count is greater than one, another thread might change
    /// the count at any time, including potentially between calling this method and acting on the
    /// result.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use hybrid_rc::{Rc, Arc};
    ///
    /// let reference = Rc::new(42);
    /// let _2nd_ref = Rc::clone(&reference);
    /// let shared_ref = Rc::to_shared(&reference);
    /// let _2nd_shared_ref = Arc::clone(&shared_ref);
    ///
    /// assert_eq!(Rc::strong_count(&reference), 4);
    /// // shared_ref only knows the count of shared references and that there is at least one
    /// // local reference, so it will show 3 instead of 4:
    /// assert_eq!(Arc::strong_count(&shared_ref), 3);
    /// ```
    #[inline]
    pub fn strong_count(this: &Self) -> usize {
        let meta = this.meta();
        meta.strong_shared.load(Ordering::SeqCst)
            + if State::SHARED {
                0
            } else {
                meta.strong_local.get() - 1
            }
    }
    
    /// Gets the approximate number of strong pointers to the pinned inner value.
    ///
    #[inline]
    pub fn strong_count_pin(this: &Pin<Self>) -> usize {
        // SAFETY: We are not moving anything and we don't expose any pointers.
        let this = unsafe { Self::pin_get_ref(this) };
        Self::strong_count(this)
    }
    
    /// Gets the number of [`Weak`] pointers to this allocation.
    ///
    /// Please understand that another thread may change the weak count at any time, including
    /// potentially between calling this method and acting on the result.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use hybrid_rc::{Rc, Weak};
    ///
    /// let reference = Rc::new(42);
    /// let weak = Rc::downgrade(&reference);
    /// let _weak_2 = weak.clone();
    ///
    /// assert_eq!(Rc::weak_count(&reference), 2);
    /// ```
    #[inline]
    pub fn weak_count(this: &Self) -> usize {
        match this.meta().weak.load(Ordering::SeqCst) {
            // Lock value => there were zero weak references apart from the implicit one.
            usize::MAX => 0,
            count => count - 1,
        }
    }
    
    /// Gets the number of [`PinWeak`] pointers to the pinned inner value.
    ///
    #[inline]
    pub fn weak_count_pin(this: &Pin<Self>) -> usize {
        // SAFETY: We are not moving anything and we don't expose any pointers.
        let this = unsafe { Self::pin_get_ref(this) };
        Self::weak_count(this)
    }
    
    // Constructs an `RcMeta` structure for a new `HybridRc` allocation
    #[inline]
    fn build_new_meta() -> RcMeta {
        RcMeta {
            owner: if State::SHARED {
                None.into()
            } else {
                ThreadId::current_thread().into()
            },
            strong_local: Cell::new(if State::SHARED { 0 } else { 1 }),
            strong_shared: 1.into(),
            weak: 1.into(),
        }
    }
    
    /// Drops the contained value and also drops the shared `RcBox` if there are no other `Weak`
    /// references.
    ///
    /// # Safety
    /// Only safe to use in `drop()` or a consuming function after verifying that no other strong
    /// reference exists. Otherwise after calling this e.g. dereferencing the `HybridRc` WILL
    /// cause undefined behaviour and even dropping it MAY cause undefined behaviour.
    unsafe fn drop_contents_and_maybe_box(&mut self) {
        // Safety: only called if this was the last strong reference
        unsafe {
            ptr::drop_in_place(Self::get_mut_unchecked(self));
        }
        
        if self.meta().dec_weak() {
            // Safety: only called if this was the last (weak) reference
            unsafe {
                RcBox::dealloc(self.ptr);
            }
        }
    }
}

impl<T, State: RcState> HybridRc<T, State> {
    /// Creates a new `Rc<T>`, moving `data` into a reference counted allocation.
    ///
    /// If `State` is `Local`, the shared value is initially owned by the calling thread, so
    /// for another thread to assume ownership [`to_shared()`] must be used and all `Rc`s for
    /// the value must be dropped.
    ///
    /// If `State` is `Shared`, initially the shared value has no owner thread, so any thread may
    /// call [`to_local()`] to assume ownership.
    ///
    /// # Examples
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let rc = Rc::new(42i32);
    /// ```
    /// ```ignore compile_fail
    /// # let rc = hybrid_rc::Rc::new(42i32);
    /// // Cannot be used in another thread without using rc.to_shared()
    /// std::thread::spawn(move || *rc).join(); // does not compile
    /// ```
    ///
    /// ```ignore
    /// use hybrid_rc::Arc;
    /// # fn main() -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
    ///
    /// let arc = Arc::new(42i32);
    ///
    /// std::thread::spawn(move || assert!(*arc == 42)).join()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`to_shared()`]: Self::to_shared
    /// [`to_local()`]: Self::to_local
    #[inline]
    pub fn new(data: T) -> Self {
        let mut inner = RcBox::allocate(Self::build_new_meta());
        let inner = unsafe { inner.as_mut() };
        inner.data.write(data);
        Self::from_inner(unsafe { inner.assume_init() }.into())
    }
    
    /// Creates a new `HybridRc` with uninitialized contents.
    #[inline]
    pub fn new_uninit() -> HybridRc<mem::MaybeUninit<T>, State> {
        let inner = RcBox::allocate(Self::build_new_meta());
        HybridRc::from_inner(inner)
    }
    
    /// Creates a new `HybridRc` with uninitialized contents, with the memory being filled with
    /// 0 bytes.
    ///
    /// See [`MaybeUninit::zeroed()`] for examples of correct and incorrect usage of this method.
    ///
    /// [`MaybeUninit::zeroed()`]: mem::MaybeUninit::zeroed
    #[inline]
    pub fn new_zeroed() -> HybridRc<mem::MaybeUninit<T>, State> {
        let mut inner = RcBox::allocate(Self::build_new_meta());
        unsafe { inner.as_mut() }.data = mem::MaybeUninit::zeroed();
        HybridRc::from_inner(inner)
    }
    
    /// Creates a new `HybridRc` with a possibly cyclic reference.
    ///
    /// For this a reference to a [`Weak`] is passed to the closure that – after this function
    /// returns – will point to the new value itself. Attempting to upgrade the weak reference
    /// before `new_cyclic` returns will result in a `ValueDropped` error. However, the weak
    /// reference may be cloned freely and stored for use at a later time.
    #[inline]
    pub fn new_cyclic(
        data_fn: impl FnOnce(&Weak<T>) -> T,
    ) -> HybridRc<T, State> {
        // Construct metadata for an initially non-upgradable RcBox
        let meta = RcMeta {
            owner: if State::SHARED {
                None.into()
            } else {
                ThreadId::current_thread().into()
            },
            strong_local: Cell::new(0),
            strong_shared: 0.into(),
            weak: 1.into(),
        };
        
        // Allocate memory (uninitialized)
        let inner = RcBox::<T>::allocate(meta);
        
        // Construct `Weak`
        let weak: Weak<T> = Weak { ptr: inner.cast() };
        
        // Run data function, keeping the ownership of the weak reference.
        let data = data_fn(&weak);
        
        // Initialize data in our box
        // Not creating an immediate &mut of the whole box to not invalidate the
        // weak pointer under Stacked Borrows rules.
        unsafe { &mut *ptr::addr_of_mut!((*inner.as_ptr()).data) }.write(data);
        
        // Don't run `Weak`s destructor. The value we just initialized should keep existing and we
        // need a weak count of 1 for the strong reference that we are currently constructing.
        mem::forget(weak);
        
        // Fix the reference counts
        {
            let meta = unsafe { &*ptr::addr_of!((*inner.as_ptr()).meta) };
            if !State::SHARED {
                meta.inc_strong_local()
            }
            // Must be at least `Release`, so that all threads see the initialized data before
            // they can observe a non-zero reference count.
            meta.strong_shared.fetch_add(1, Ordering::Release);
        }
        
        Self::from_inner(inner.cast())
    }
    
    /// Creates a new `Pin<HybridRc<T>>`. If `T` does not implement `Unpin`, then `data` will be
    /// pinned in memory and unable to be moved.
    #[inline]
    pub fn pin(data: T) -> Pin<Self> {
        unsafe { Pin::new_unchecked(Self::new(data)) }
    }
    
    /// Tries to creates a new `Rc<T>`, moving `data` into a reference counted allocation.
    ///
    /// # Errors
    /// Will drop `data` and return `Err(`[`AllocError`]`)` if the allocation fails.
    ///
    /// Please note that the global allocator on some systems may instead abort the process if an
    /// allocation failure happens.
    #[inline]
    pub fn try_new(data: T) -> Result<Self, AllocError> {
        let mut inner = RcBox::try_allocate(Self::build_new_meta())
            .map_err(|_| AllocError)?;
        let inner = unsafe { inner.as_mut() };
        inner.data.write(data);
        Ok(Self::from_inner(unsafe { inner.assume_init() }.into()))
    }
    
    /// Tries to construct a new `HybridRc` with uninitialized contents.
    ///
    /// # Errors
    /// Will return `Err(`[`AllocError`]`)` if the allocation fails.
    ///
    /// Please note that the global allocator on some systems may instead abort the process if an
    /// allocation failure happens.
    #[inline]
    pub fn try_new_uninit(
    ) -> Result<HybridRc<mem::MaybeUninit<T>, State>, AllocError> {
        let inner = RcBox::try_allocate(Self::build_new_meta())
            .map_err(|_| AllocError)?;
        Ok(HybridRc::from_inner(inner))
    }
    
    /// Tries to construct a new `HybridRc` with uninitialized contents, with the memory being
    /// filled with 0 bytes.
    ///
    /// See [`MaybeUninit::zeroed()`] for examples of correct and incorrect usage of this method.
    ///
    /// # Errors
    /// Will return `Err(`[`AllocError`]`)` if the allocation fails.
    ///
    /// Please note that the global allocator on some systems may instead abort the process if an
    /// allocation failure happens.
    ///
    /// [`MaybeUninit::zeroed()`]: mem::MaybeUninit::zeroed
    #[inline]
    pub fn try_new_zeroed(
    ) -> Result<HybridRc<mem::MaybeUninit<T>, State>, AllocError> {
        let mut inner = RcBox::try_allocate(Self::build_new_meta())
            .map_err(|_| AllocError)?;
        unsafe { inner.as_mut() }.data = mem::MaybeUninit::zeroed();
        Ok(HybridRc::from_inner(inner))
    }
    
    /// Returns the inner value, if this `HybridRc` is the only strong reference to it.
    ///
    /// Any outstanding [`Weak`] references won't be able to upgrade anymore when this succeeds.
    ///
    /// # Errors
    /// If this is not the only strong reference to the shared value, an [`Err`] is returned with
    /// the same `HybridRc` that was passed in.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let reference = Rc::new(42);
    /// let weak = Rc::downgrade(&reference);
    ///
    /// let value = Rc::try_unwrap(reference).unwrap();
    /// assert_eq!(value, 42);
    /// assert!(weak.upgrade().is_err()); // Weaks cannot upgrade anymore.
    /// ```
    #[inline]
    pub fn try_unwrap(this: Self) -> Result<T, Self> {
        if State::SHARED {
            Self::try_unwrap_internal(this)
        } else {
            // If we may access the local counter, first check and decrement that one.
            let local_count = this.meta().strong_local.get();
            if local_count == 1 {
                this.meta().strong_local.set(0);
                match Self::try_unwrap_internal(this) {
                    Ok(result) => Ok(result),
                    Err(this) => {
                        this.meta().strong_local.set(local_count);
                        Err(this)
                    }
                }
            } else {
                Err(this)
            }
        }
    }
    
    /// Returns the inner value, if this `HybridRc` is the only strong reference to it, assuming
    /// that there are no (other) local references to the value.
    ///
    /// Used internally by `try_unwrap()`.
    #[inline]
    fn try_unwrap_internal(this: Self) -> Result<T, Self> {
        let meta = this.meta();
        // There is one implicit shared reference for all local references, so if there are no other
        // local references or we are a shared shared and the shared counter is 1, we are the only
        // strong reference left.
        if meta
            .strong_shared
            .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            Err(this)
        } else {
            // Relaxed should be enough, as `strong_shared` already hit 0, so no more
            // Weak upgrading is possible.
            meta.owner.store(None, Ordering::Relaxed);
            
            let copy = unsafe { ptr::read(Self::as_ptr(&this)) };
            
            // Make a weak pointer to clean up the remaining implicit weak reference
            let _weak = Weak { ptr: this.ptr };
            mem::forget(this);
            
            Ok(copy)
        }
    }
}

impl<T, State: RcState> HybridRc<[T], State> {
    /// Creates a new reference-counted slice with uninitialized contents.
    #[inline]
    pub fn new_uninit_slice(
        len: usize,
    ) -> HybridRc<[mem::MaybeUninit<T>], State> {
        let inner = RcBox::allocate_slice(Self::build_new_meta(), len, false);
        HybridRc::from_inner(inner.into())
    }
    
    /// Creates a new reference-counted slice with uninitialized contents, with the memory being
    /// filled with 0 bytes.
    #[inline]
    pub fn new_zeroed_slice(
        len: usize,
    ) -> HybridRc<[mem::MaybeUninit<T>], State> {
        let inner = RcBox::allocate_slice(Self::build_new_meta(), len, true);
        HybridRc::from_inner(inner.into())
    }
    
    /// Copies the contents of a slice into a new `HybridRc`
    ///
    /// # Safety
    /// Either `T` is `Copy` or the caller must guarantee that the the source doesn't drop its
    /// contents.
    #[inline]
    unsafe fn copy_from_slice_unchecked(src: &[T]) -> Self {
        let len = src.len();
        let inner = RcBox::allocate_slice(Self::build_new_meta(), len, false);
        let dest = ptr::addr_of_mut!(inner.data).cast();
        
        // Safety: The freshly allocated `RcBox` can't alias `src` and the payload can be fully
        // initialized by copying the slice memory. The copying is also safe as long as the safety
        // requirements for calling this are fulfilled.
        unsafe {
            src.as_ptr().copy_to_nonoverlapping(dest, src.len());
            HybridRc::from_inner(inner.assume_init().into())
        }
    }
}

impl<T: Copy, State: RcState> HybridRc<[T], State> {
    /// Copies the contents of a slice into a new `HybridRc`
    ///
    /// Optimization for copyable types. Will become deprecated once specialization is stablilized.
    #[inline]
    pub fn copy_from_slice(src: &[T]) -> Self {
        // Safety: `T` is `Copy`.
        unsafe { Self::copy_from_slice_unchecked(src) }
    }
}

impl<T: ?Sized> Rc<T> {
    /// Creates a new shared reference (`Arc`) for the referenced value.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::{Rc, Arc};
    /// # fn main() -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
    ///
    /// let local = Rc::new(42i32);
    /// let shared = Rc::to_shared(&local);
    ///
    /// // `shared` can be safely transferred to another thread
    /// std::thread::spawn(move || assert_eq!(*shared, 42i32)).join()?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn to_shared(this: &Self) -> Arc<T> {
        this.meta().inc_strong_shared();
        Arc::from_inner(this.ptr)
    }
    
    /// Creates a new pinned shared reference for the referenced value.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::{Rc, Weak};
    ///
    /// let strong = Rc::pin(42i32);
    /// let shared = Rc::to_shared_pin(&strong);
    /// assert!(Rc::ptr_eq_pin(&strong, &shared));
    /// ```
    #[inline]
    pub fn to_shared_pin(this: &Pin<Self>) -> Pin<Arc<T>> {
        // SAFETY: We are not moving anything, we don't expose a non-pinned pointer,
        // and we create a Pin-wrapper only for a pinned value.
        unsafe {
            let this = Self::pin_get_ref(this);
            Pin::new_unchecked(Self::to_shared(this))
        }
    }
    
    /// Increments the local strong reference count on the `Rc<T>` associated by the given pointer
    ///
    /// Increases the local strong reference count as if a new `Rc` was cloned and kept alive.
    /// May panic in the unlikely case the platform-specific maximum for the reference count is
    /// reached.
    ///
    /// # Safety
    /// The pointer must have been obtained through [`HybridRc<T, Local>::into_raw()`], the value
    /// must still be live and have a local strong count of at least 1 when this method is invoked
    /// and this call must be performed on the same thread as where the original `Rc` was created.
    ///
    /// [`HybridRc<T, Local>::into_raw()`]: `Rc::into_raw`
    #[inline]
    pub unsafe fn increment_local_strong_count(ptr: *const T) {
        unsafe {
            let box_ptr = RcBox::<T>::ptr_from_data_ptr(ptr as *mut T);
            (*box_ptr).meta.inc_strong_local();
        }
    }
    
    /// Decrements the local strong reference count on the `Rc<T>` associated by the given pointer
    ///
    /// If the local strong reference counter reaches 0, the value is no longer considered owned
    /// by the calling thread and if there are no shared strong references to keep the value alive,
    /// it will be dropped.
    ///
    /// # Safety
    /// The pointer must have been obtained through [`HybridRc<T, Local>::into_raw()`], the value
    /// must still be live and have a local strong count of at least 1 when this method is invoked
    /// and this call must be performed on the same thread as where the original `Rc` was created.
    ///
    /// [`HybridRc<T, Local>::into_raw()`]: `Rc::into_raw`
    #[inline]
    pub unsafe fn decrement_local_strong_count(ptr: *const T) {
        mem::drop(unsafe { Rc::from_raw(ptr) });
    }
}

impl<T: ?Sized> Arc<T> {
    /// Creates a new local reference (`Rc`) for the referenced value.
    ///
    /// Returns `None` if at least one `Rc` already exists on another thread.
    ///
    /// **Note:** In `no_std` environments `None` is returned if at least one `Rc` exists on *any*
    /// thread.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::{Rc, Arc};
    /// # fn main() -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
    ///
    /// let local = Rc::new(42i32);
    /// let shared = Rc::to_shared(&local);
    ///
    /// // `shared` can be safely transferred to another thread
    /// std::thread::spawn(move || assert_eq!(*shared, 42i32)).join()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    #[inline]
    pub fn to_local(this: &Self) -> Option<Rc<T>> {
        let meta = this.meta();
        let current_thread = ThreadId::current_thread();
        let owner = match meta.owner.store_if_none(
            Some(current_thread),
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => None,
            Err(owner) => owner,
        };
        
        match owner {
            None => {
                meta.try_inc_strong_local()
                    .expect("inconsistent reference count (shared == 0)");
                Some(Rc::from_inner(this.ptr))
            }
            Some(v) if v == current_thread => {
                meta.inc_strong_local();
                Some(Rc::from_inner(this.ptr))
            }
            Some(_) => None,
        }
    }
    
    /// Creates a new pinned local reference for the referenced value.
    ///
    /// Returns `None` if at least one `Rc` already exists on another thread.
    ///
    /// **Note:** In `no_std` environments `None` is returned if at least one `Rc` exists on *any*
    /// thread.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::{Arc, Weak};
    ///
    /// let strong = Arc::pin(42i32);
    /// let local = Arc::to_local_pin(&strong).unwrap();
    /// assert!(Arc::ptr_eq_pin(&strong, &local));
    /// ```
    #[must_use]
    #[inline]
    pub fn to_local_pin(this: &Pin<Self>) -> Option<Pin<Rc<T>>> {
        // SAFETY: We are not moving anything, we don't expose a non-pinned pointer,
        // and we create a Pin-wrapper only for a pinned value.
        unsafe {
            let this = Self::pin_get_ref(this);
            Some(Pin::new_unchecked(Self::to_local(this)?))
        }
    }
    
    /// Increments the shared strong reference count on the `Arc<T>` associated by the given pointer
    ///
    /// Increases the shared strong reference count as if a new `Arc` was cloned and kept alive.
    /// May panic in the unlikely case the platform-specific maximum for the reference count is
    /// reached.
    ///
    /// # Safety
    /// The pointer must have been obtained through [`HybridRc<T, Shared>::into_raw()`] and the
    /// value must still be live when this method is invoked.
    ///
    /// [`HybridRc<T, Shared>::into_raw()`]: `Arc::into_raw`
    #[inline]
    pub unsafe fn increment_shared_strong_count(ptr: *const T) {
        unsafe {
            let box_ptr = RcBox::<T>::ptr_from_data_ptr(ptr);
            (*box_ptr).meta.inc_strong_shared();
        }
    }
    
    /// Decrements the shared strong reference count on the `Arc<T>` associated by the given pointer
    ///
    /// If the shared strong reference counter (including the implicit shared reference for local
    /// strong references) reaches 0, the value will be dropped.
    ///
    /// # Safety
    /// The pointer must have been obtained through [`HybridRc<T, Shared>::into_raw()`] and the
    /// value must still be live when this method is invoked.
    ///
    /// [`HybridRc<T, Shared>::into_raw()`]: `Arc::into_raw`
    #[inline]
    pub unsafe fn decrement_shared_strong_count(ptr: *const T) {
        mem::drop(unsafe { Arc::from_raw(ptr) });
    }
}

impl<T: Clone, State: RcState> HybridRc<T, State> {
    /// Makes a mutable reference into the given `HybridRc`.
    ///
    /// If there are other strong references to the same value, then `make_mut()` will [`clone`] the
    /// inner value to a new allocation to ensure unique ownership.  This is also referred to as
    /// clone-on-write.
    ///
    /// However, if there are no other strong references to this allocation, but some [`Weak`]
    /// pointers, then the [`Weak`]s will be disassociated and the inner value will not be cloned.
    ///
    /// See also: [`get_mut()`], which will fail rather than cloning the inner value
    /// or diassociating [`Weak`]s.
    ///
    /// [`clone`]: Clone::clone
    /// [`get_mut()`]: HybridRc::get_mut
    ///
    /// # Example
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let mut reference = Rc::new(42);
    ///
    /// *Rc::make_mut(&mut reference) += 2;          // Won't clone anything
    /// let mut reference_2 = Rc::clone(&reference); // Won't clone inner value
    /// *Rc::make_mut(&mut reference) += 1;         // Clones inner data
    /// *Rc::make_mut(&mut reference) *= 2;        // Won't clone anything
    /// *Rc::make_mut(&mut reference_2) /= 4;       // Won't clone anything
    ///
    /// // Now `reference` and `reference_2` point to different allocations.
    /// assert_eq!(*reference, 90);
    /// assert_eq!(*reference_2, 11);
    /// ```
    #[inline]
    pub fn make_mut(this: &mut Self) -> &mut T {
        let meta = this.meta();
        if State::SHARED {
            Self::make_mut_internal(this, false)
        } else {
            let local_count = meta.strong_local.get();
            Self::make_mut_internal(this, local_count > 1)
        }
    }
    
    /// Makes a mutable reference into the given `HybridRc`, assuming that only the shared strong
    /// counter needs to be checked.
    ///
    /// If `force_clone` is true, the counters are ignored and uniqueness will always be ensured
    /// by cloning the shared allocation.
    ///
    /// Used internally by `make_mut()`.
    #[inline]
    fn make_mut_internal(this: &mut Self, force_clone: bool) -> &mut T {
        let meta = this.meta();
        // There is one implicit shared reference for all local references, so if there are no other
        // local references or we are a shared shared and the shared counter is 1, we are the only
        // strong reference left.
        if force_clone
            || meta
                .strong_shared
                .compare_exchange(1, 0, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
        {
            // Clone the allocation and make `this` point to the new clone
            let mut donor = this.clone_allocation();
            mem::swap(&mut this.ptr, &mut donor.ptr);
        } else {
            // Check if there are Weak references left.
            // Relaxed suffices, as if there is a race with a dropping Weak, then it's only a
            // missing optimization, but the code keeps being sound.
            if meta.weak.load(Ordering::Relaxed) != 1 {
                // Acts as a guard to decrement the weak counter
                let _weak = Weak { ptr: this.ptr };
                
                // Steal the payload data
                let mut donor = Self::new_uninit();
                unsafe {
                    let uninit = HybridRc::get_mut_unchecked(&mut donor);
                    uninit.as_mut_ptr().copy_from_nonoverlapping(&**this, 1);
                    let donor = donor.assume_init();
                    this.ptr = donor.ptr;
                    mem::forget(donor);
                }
            } else {
                // There were no Weak references, so we are the unique reference. Bump the counter
                // back up.
                meta.strong_shared.store(1, Ordering::Release);
            }
        }
        
        // Safe, because by now we are the only reference to the allocation in `this.ptr`, either
        // to begin with, by swapping or by stealing.
        unsafe { Self::get_mut_unchecked(this) }
    }
    
    /// Clones the shared allocation and returns a `HybridRc` pointing to the clone.
    #[inline]
    fn clone_allocation(&self) -> Self {
        let mut result = Self::new_uninit();
        let uninit = unsafe { HybridRc::get_mut_unchecked(&mut result) };
        uninit.write((*self.data()).clone());
        unsafe { result.assume_init() }
    }
    
    /// Returns the inner value, if this `HybridRc` is the only strong reference to it and else
    /// a clone of it.
    ///
    /// Any outstanding [`Weak`] references won't be able to upgrade anymore when this was the only
    /// strong reference.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let rc = Rc::new(vec![1,2,3]);
    /// let ptr = (*rc).as_ptr();
    /// let weak = Rc::downgrade(&rc);
    /// let inner = Rc::unwrap_or_clone(rc); // No other strong reference exists, so this doesn't clone.
    /// assert_eq!(ptr, inner.as_ptr());
    /// assert!(weak.upgrade().is_err()); // The remaining weak reference cannot upgrade anymore.
    ///
    /// let rc = Rc::new(vec![1,2,3,4]);
    /// let ptr = (*rc).as_ptr();
    /// let rc2 = Rc::clone(&rc);
    /// let inner = Rc::unwrap_or_clone(rc); // `rc2` exists, so this clones the value.
    /// assert_ne!(ptr, inner.as_ptr());
    /// assert_eq!(ptr, (*rc2).as_ptr()); // `rc2` still points to the original value.
    /// ```
    #[inline]
    pub fn unwrap_or_clone(this: Self) -> T {
        Self::try_unwrap(this).unwrap_or_else(|this| (*this).clone())
    }
}

impl<T, State: RcState> HybridRc<mem::MaybeUninit<T>, State> {
    /// Assumes the value is initialized and converts to `HybridRc<T, State>`.
    ///
    /// # Safety
    ///
    /// You need to provide the same guarantees as for [`MaybeUninit::assume_init()`].
    /// Calling this when the value is not yet fully initialized causes immediate undefined
    /// behavior.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let mut reference = Rc::<i64>::new_uninit();
    ///
    /// let reference = unsafe {
    ///     // Deferred initialization
    ///     Rc::get_mut_unchecked(&mut reference).as_mut_ptr().write(1337);
    ///     reference.assume_init()
    /// };
    ///
    /// assert_eq!(*reference, 1337)
    /// ```
    ///
    /// [`MaybeUninit::assume_init()`]: mem::MaybeUninit::assume_init
    #[inline]
    pub unsafe fn assume_init(self) -> HybridRc<T, State> {
        HybridRc::from_inner(mem::ManuallyDrop::new(self).ptr.cast())
    }
}

impl<T, State: RcState> HybridRc<[mem::MaybeUninit<T>], State> {
    /// Assumes the values are initialized and converts to `HybridRc<[T], State>`.
    ///
    /// # Safety
    ///
    /// You need to provide the same guarantees as for [`MaybeUninit::assume_init()`].
    /// Calling this when the whole slice is not yet fully initialized causes immediate undefined
    /// behavior.
    ///
    /// [`MaybeUninit::assume_init()`]: mem::MaybeUninit::assume_init
    #[inline]
    pub unsafe fn assume_init(self) -> HybridRc<[T], State> {
        HybridRc::from_inner(unsafe {
            mem::ManuallyDrop::new(self)
                .ptr
                .as_mut()
                .assume_init()
                .into()
        })
    }
}

impl<State: RcState> HybridRc<dyn Any, State> {
    /// Tries to downcast the `HybridRc<dyn Any, _>` to a concrete type.
    ///
    /// # Errors
    /// If a downcast failed, the original `HybridRc` is returned in `Err`
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::any::Any;
    /// use std::mem::drop;
    /// use hybrid_rc::Rc;
    ///
    /// let value = 42i32;
    /// let concrete = Rc::new(value);
    /// let any: Rc<dyn Any> = Rc::into(concrete);
    ///
    /// let any = any.downcast::<String>().unwrap_err();
    ///
    /// assert_eq!(*any.downcast::<i32>().unwrap(), 42);
    /// ```
    #[inline]
    pub fn downcast<T: Any>(self) -> Result<HybridRc<T, State>, Self> {
        if (*self).is::<T>() {
            let ptr = self.ptr.cast::<RcBox<T>>();
            mem::forget(self);
            Ok(HybridRc::from_inner(ptr))
        } else {
            Err(self)
        }
    }
}

impl<State: RcState> HybridRc<dyn Any + Sync + Send, State> {
    /// Tries to downcast the `HybridRc<dyn Any + Sync + Send, _>` to a concrete type.
    ///
    /// # Errors
    /// If a downcast failed, the original `HybridRc` is returned in `Err`
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::any::Any;
    /// use std::mem::drop;
    /// use hybrid_rc::Rc;
    ///
    /// let value = 42i32;
    /// let concrete = Rc::new(value);
    /// let any: Rc<dyn Any + Sync + Send> = Rc::into(concrete);
    ///
    /// let any = any.downcast::<String>().unwrap_err();
    ///
    /// assert_eq!(*any.downcast::<i32>().unwrap(), 42);
    /// ```
    #[inline]
    pub fn downcast<T: Any + Sync + Send>(
        self,
    ) -> Result<HybridRc<T, State>, Self> {
        if (*self).is::<T>() {
            let ptr = self.ptr.cast::<RcBox<T>>();
            mem::forget(self);
            Ok(HybridRc::from_inner(ptr))
        } else {
            Err(self)
        }
    }
}

impl<T: ?Sized> Clone for HybridRc<T, Local> {
    /// Creates another `Rc` for the same value.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let first = Rc::new(42i32);
    /// let second = Rc::clone(&first);
    ///
    /// assert_eq!(Rc::as_ptr(&first), Rc::as_ptr(&second));
    /// ```
    #[inline]
    fn clone(&self) -> Self {
        self.meta().inc_strong_local();
        Self::from_inner(self.ptr)
    }
}

impl<T: ?Sized> Clone for HybridRc<T, Shared> {
    /// Creates another `Arc` for the same value.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::Arc;
    /// # fn main() -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
    ///
    /// let first = Arc::new(42i32);
    /// let second = Arc::clone(&first);
    ///
    /// assert_eq!(Arc::as_ptr(&first), Arc::as_ptr(&second));
    ///
    /// let value = std::thread::spawn(move || *second)
    ///   .join()?;
    /// assert_eq!(*first, value);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    fn clone(&self) -> Self {
        self.meta().inc_strong_shared();
        Self::from_inner(self.ptr)
    }
}

impl<T: ?Sized, State: RcState> Drop for HybridRc<T, State> {
    /// Drops the `HybridRc`.
    ///
    /// This will decrement the appropriate reference count depending on `State`. If both strong
    /// reference counts reach zero then the only other references (if any) are [`Weak`]. In that
    /// case the inner value is dropped.
    #[inline]
    fn drop(&mut self) {
        let no_more_strong_refs = if State::SHARED {
            self.meta().dec_strong_shared()
        } else {
            self.meta().dec_strong_local()
        };
        
        if no_more_strong_refs {
            unsafe {
                self.drop_contents_and_maybe_box();
            }
        }
    }
}

// Dereferencing traits

impl<T: ?Sized, State: RcState> Deref for HybridRc<T, State> {
    type Target = T;
    
    #[inline]
    fn deref(&self) -> &T {
        self.data()
    }
}

impl<T: ?Sized, State: RcState> Borrow<T> for HybridRc<T, State> {
    #[inline]
    fn borrow(&self) -> &T {
        self
    }
}

impl<T: ?Sized, State: RcState> AsRef<T> for HybridRc<T, State> {
    #[inline]
    fn as_ref(&self) -> &T {
        self
    }
}

// Safety: T: Sync implies that dereferencing the Arc<T> on multiple threads is sound and T: Send
// implies that dropping T on another thread is sound. So T: Sync + Send gives all guarantees we
// need to make Arc Sync + Send.
unsafe impl<T: ?Sized + Sync + Send> Send for HybridRc<T, Shared> {}
unsafe impl<T: ?Sized + Sync + Send> Sync for HybridRc<T, Shared> {}

// Unwind safety: A HybridRc can only be UnwindSafe if the inner type is RefUnwindSafe.
impl<T: RefUnwindSafe + ?Sized, State: RcState> UnwindSafe
    for HybridRc<T, State>
{
}

// Unwind safety: An Arc is always RefUnwindSafe because of its use of atomics.
impl<T: RefUnwindSafe> RefUnwindSafe for HybridRc<T, Shared> {}

// Conversions between different HybridRc variants

impl<T: Any + 'static, State: RcState> From<HybridRc<T, State>>
    for HybridRc<dyn Any + 'static, State>
{
    /// Upcasts a `HybridRc<T, State>` into a `HybridRc<dyn Any, State>`
    #[inline]
    fn from(src: HybridRc<T, State>) -> Self {
        let ptr = src.ptr.as_ptr() as *mut RcBox<dyn Any>;
        mem::forget(src);
        Self::from_inner(unsafe { NonNull::new_unchecked(ptr) })
    }
}

impl<T: Any + Sync + Send + 'static, State: RcState> From<HybridRc<T, State>>
    for HybridRc<dyn Any + Sync + Send + 'static, State>
{
    /// Upcasts a `HybridRc<T, State>` into a `HybridRc<dyn Any + Sync + Send, State>`
    #[inline]
    fn from(src: HybridRc<T, State>) -> Self {
        let ptr = src.ptr.as_ptr() as *mut RcBox<dyn Any + Sync + Send>;
        mem::forget(src);
        Self::from_inner(unsafe { NonNull::new_unchecked(ptr) })
    }
}

impl<T, State: RcState, const N: usize> From<HybridRc<[T; N], State>>
    for HybridRc<[T], State>
{
    /// Converts a `HybridRc<[T; N], State>` into a `HybridRc<[T], State>`
    ///
    /// Workaround for coercion as long as `CoerceUnsized` is unstable.
    #[inline]
    fn from(src: HybridRc<[T; N], State>) -> Self {
        let ptr = src.ptr.as_ptr() as *mut RcBox<[T]>;
        mem::forget(src);
        Self::from_inner(unsafe { NonNull::new_unchecked(ptr) })
    }
}

impl<T: ?Sized> From<Rc<T>> for HybridRc<T, Shared> {
    /// Converts an `Rc<T>` into an `Arc<T>`.
    ///
    /// See [`to_shared()`].
    ///
    /// [`to_shared()`]: HybridRc::to_shared
    #[inline]
    fn from(src: Rc<T>) -> Self {
        HybridRc::to_shared(&src)
    }
}

impl<T: ?Sized> TryFrom<Arc<T>> for HybridRc<T, Local> {
    type Error = Arc<T>;
    
    /// Tries to convert an `Arc<T>` into an `Rc<T>`.
    ///
    /// See [`to_local()`].
    ///
    /// [`to_local()`]: HybridRc::to_local
    #[inline]
    fn try_from(src: Arc<T>) -> Result<Self, Self::Error> {
        match HybridRc::to_local(&src) {
            Some(result) => Ok(result),
            None => Err(src),
        }
    }
}

impl<T, State: RcState, const N: usize> TryFrom<HybridRc<[T], State>>
    for HybridRc<[T; N], State>
{
    type Error = HybridRc<[T], State>;
    
    /// Tries to convert a `HybridRc<[T], State>` into a `HybridRc<[T; N], State>`
    ///
    /// Only succeeds if the length matches exactly.
    #[inline]
    fn try_from(src: HybridRc<[T], State>) -> Result<Self, Self::Error> {
        if src.len() == N {
            let ptr = src.ptr.as_ptr().cast();
            mem::forget(src);
            Ok(Self::from_inner(unsafe { NonNull::new_unchecked(ptr) }))
        } else {
            Err(src)
        }
    }
}

// Conversions into HybridRc

impl<T, State: RcState> From<T> for HybridRc<T, State> {
    /// Moves a `T` into an `HybridRc<T, State>`
    ///
    /// Equivalent to calling [`HybridRc::new(src)`].
    ///
    /// [`HybridRc::new(t)`]: Self::new
    #[inline]
    fn from(src: T) -> Self {
        Self::new(src)
    }
}

impl<T: Clone, State: RcState> From<&[T]> for HybridRc<[T], State> {
    /// Allocate a reference-counted slice and clone the elements of `src` into it.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let vecs = [
    ///     vec![1,2,3],
    ///     vec![4,5,6],
    /// ];
    /// let rc: Rc<[_]> = Rc::from(&vecs[..]);
    /// assert_eq!(&rc[..], &vecs);
    /// ```
    #[inline]
    fn from(src: &[T]) -> Self {
        let mut builder = SliceBuilder::new(Self::build_new_meta(), src.len());
        for item in src {
            builder.append(Clone::clone(item));
        }
        Self::from_inner(builder.finish().into())
    }
}

impl<T, State: RcState> From<Vec<T>> for HybridRc<[T], State> {
    /// Allocate a reference-counted slice and move `src`'s items into it.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let vec = vec!["a","b","c"];
    /// let rc: Rc<[_]> = Rc::from(vec);
    /// assert_eq!(&rc[..], &["a", "b", "c"]);
    /// ```
    #[inline]
    fn from(mut src: Vec<T>) -> Self {
        unsafe {
            let result =
                HybridRc::<_, State>::copy_from_slice_unchecked(&src[..]);
            
            // Set the length of `src`, so that the moved items are not dropped.
            src.set_len(0);
            
            result
        }
    }
}

impl<State: RcState> From<&str> for HybridRc<str, State> {
    /// Allocate a reference-counted `str` and copy `src` into it.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let a: Rc<str> = Rc::from("foobar");
    /// assert_eq!(&a[..], "foobar");
    /// ```
    #[inline]
    fn from(src: &str) -> Self {
        let bytes = HybridRc::<_, State>::copy_from_slice(src.as_bytes());
        let inner = unsafe {
            (bytes.ptr.as_ptr() as *mut _ as *mut RcBox<str>).as_mut()
        }
        .unwrap();
        mem::forget(bytes);
        Self::from_inner(inner.into())
    }
}

impl<State: RcState> From<String> for HybridRc<str, State> {
    /// Allocate a reference-counted `str` and copy `src` into it.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let string: String = "foobar".to_owned();
    /// let a: Rc<str> = Rc::from(string);
    /// assert_eq!(&a[..], "foobar");
    /// ```
    #[inline]
    fn from(src: String) -> Self {
        Self::from(&src[..])
    }
}

impl<'a, T: ToOwned + ?Sized, State: RcState> From<Cow<'a, T>>
    for HybridRc<T, State>
where
    HybridRc<T, State>: From<&'a T> + From<T::Owned>,
{
    /// Creates a new `HybridRc<T, State>` from a clone-on-write pointer by copying its content.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use hybrid_rc::Rc;
    /// use std::borrow::Cow;
    ///
    /// let cow: Cow<str> = Cow::Borrowed("foobar");
    /// let a: Rc<str> = Rc::from(cow);
    /// assert_eq!(&a[..], "foobar");
    /// ```
    #[inline]
    fn from(src: Cow<'a, T>) -> HybridRc<T, State> {
        match src {
            Cow::Borrowed(value) => HybridRc::from(value),
            Cow::Owned(value) => HybridRc::from(value),
        }
    }
}

impl<T: ?Sized, State: RcState> From<Box<T>> for HybridRc<T, State> {
    #[inline]
    fn from(src: Box<T>) -> HybridRc<T, State> {
        let len = mem::size_of_val(&*src);
        let inner =
            RcBox::allocate_for_val(Self::build_new_meta(), &*src, false);
        let dest = unsafe { ptr::addr_of_mut!((*inner.as_ptr()).data) }.cast();
        
        // Safety: The freshly allocated `RcBox` can't alias `src` and the payload can be fully
        // moved by copying the memory, because it's not Pin<Box<T>>. `allocate_for_val` ensures
        // the destination payload buffer is big enough for the value.
        unsafe {
            (&*src as *const T)
                .cast::<u8>()
                .copy_to_nonoverlapping(dest, len);
        }
        
        // Drop original box without running the destructor
        // Safety: This *should* be sound, as ManuallyDrop<T> has the same layout as T.
        mem::drop(unsafe {
            mem::transmute::<Box<T>, Box<mem::ManuallyDrop<T>>>(src)
        });
        
        HybridRc::from_inner(inner)
    }
}

impl<T, State: RcState> iter::FromIterator<T> for HybridRc<[T], State> {
    /// Takes each element in the `Iterator` and collects it into an `HybridRc<[T], State>`.
    ///
    /// # Performance characteristics
    ///
    /// Collecion is done by first collecting into a `Vec<T>`.
    ///
    /// This will allocate as many times as needed for constructing the `Vec<T>`
    /// and then it will allocate once for turning the `Vec<T>` into the `HybridRc<[T], State>`.
    ///
    /// Once specialization is stablilized this will be optimized for [`TrustedLen`] iterators.
    ///
    /// [`TrustedLen`]: core::iter::TrustedLen
    fn from_iter<I: iter::IntoIterator<Item = T>>(iter: I) -> Self {
        let vec: Vec<T> = iter.into_iter().collect();
        vec.into()
    }
}

// Propagate some useful traits implemented by the inner type

impl<T: Default, State: RcState> Default for HybridRc<T, State> {
    /// Creates a new `HybridRc`, with the `Default` value for `T`.
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T: ?Sized + PartialEq, S1: RcState, S2: RcState> PartialEq<HybridRc<T, S2>>
    for HybridRc<T, S1>
{
    /// Equality for `HybridRc`s.
    ///
    /// Two `HybridRc`s are equal if their inner values are equal, independent of if they are
    /// stored in the same allocation.
    #[inline]
    fn eq(&self, other: &HybridRc<T, S2>) -> bool {
        **self == **other
    }
}

impl<T: ?Sized + Eq, State: RcState> Eq for HybridRc<T, State> {}

impl<T: ?Sized + Hash, State: RcState> Hash for HybridRc<T, State> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        Self::data(self).hash(state);
    }
}

impl<T: ?Sized + PartialOrd, S1: RcState, S2: RcState>
    PartialOrd<HybridRc<T, S2>> for HybridRc<T, S1>
{
    /// Partial comparison for `HybridRc`s.
    ///
    /// The two are compared by calling `partial_cmp()` on their inner values.
    #[inline]
    fn partial_cmp(&self, other: &HybridRc<T, S2>) -> Option<cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<T: ?Sized + Ord, State: RcState> Ord for HybridRc<T, State> {
    /// Comparison for `HybridRc`s.
    ///
    /// The two are compared by calling `cmp()` on their inner values.
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl<T: ?Sized + fmt::Display, State: RcState> fmt::Display
    for HybridRc<T, State>
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&Self::data(self), f)
    }
}

impl<T: ?Sized + fmt::Debug, State: RcState> fmt::Debug for HybridRc<T, State> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&Self::data(self), f)
    }
}

// `HybridRc` can be formatted as a pointer.
impl<T: ?Sized, State: RcState> fmt::Pointer for HybridRc<T, State> {
    /// Formats the value using the given formatter.
    ///
    /// If the `#` flag is used, the state (shared/local) is written after the address.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            fmt::Pointer::fmt(&Self::as_ptr(self), f)?;
            f.write_str(if State::SHARED {
                " [shared]"
            } else {
                " [local]"
            })
        } else {
            fmt::Pointer::fmt(&Self::as_ptr(self), f)
        }
    }
}

/// `HybridRc<T>` is always `Unpin` itself, because the data value is on the heap,
/// so moving `HybridRc<T>` doesn't move the content even if `T` is not `Unpin`.
///
/// This allows unpinning e.g. `Pin<Box<HybridRc<T>>>` but not any `Pin<HybridRc<T>>`!
impl<T: ?Sized, State: RcState> Unpin for HybridRc<T, State> {}

/// `Weak<T>` represents a non-owning reference to a value managed by a [`HybridRc<T, _>`].
/// The value is accessed by calling [`upgrade()`] or [`upgrade_local()`] on `Weak`.
///
/// `Weak` references are typically used to prevent circular references that would keep
/// the shared value alive indefinitely.
///
/// The typical way to obtain a `Weak<T>` is to call [`HybridRc::downgrade()`].
///
/// [`upgrade()`]: Weak::upgrade
/// [`upgrade_local()`]: Weak::upgrade_local
#[must_use]
pub struct Weak<T: ?Sized> {
    ptr: NonNull<RcBox<T>>,
}

impl<T: ?Sized> Weak<T> {
    /// Accesses the metadata area of the shared allocation.
    ///
    /// `None` for instances created through `Weak::new()`.
    #[inline]
    fn meta(&self) -> Option<&RcMeta> {
        if is_senitel(self.ptr.as_ptr()) {
            None
        } else {
            // Safety: as long as one Rc or Weak
            // for this item exists, the memory stays
            // allocated.
            Some(unsafe { &(*self.ptr.as_ptr()).meta })
        }
    }
    
    /// Returns a raw pointer to the value referenced by this `Weak<T>`.
    ///
    /// The pointer is valid only if there are some strong references. It may be dangling,
    /// unaligned or even null otherwise.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::Rc;
    ///
    /// let strong = Rc::new(42i32);
    /// let weak = Rc::downgrade(&strong);
    /// {
    ///     let pointer = weak.as_ptr();
    ///     // As long as strong is not dropped, the pointer stays valid
    ///     assert_eq!(42, unsafe { *pointer });
    /// }
    /// drop(strong);
    /// {
    ///     // Calling weak.as_ptr() is still safe, but dereferencing it would lead
    ///     // to undefined behaviour.
    ///     let pointer = weak.as_ptr();
    ///     // assert_eq!(42, unsafe { &*pointer }); // undefined behaviour
    /// }
    #[must_use]
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        let ptr: *mut RcBox<T> = self.ptr.as_ptr();
        
        if is_senitel(ptr) {
            // If the pointer is dangling, we return the sentinel directly. This cannot be
            // a valid payload address, as the payload is at least as aligned as ArcInner (usize).
            ptr as *const T
        } else {
            // Safety: raw pointer manipulation like in sync::Weak, as the payload may have been
            // dropped at this point and to keep provenance.
            unsafe { ptr::addr_of_mut!((*ptr).data) }
        }
    }
    
    /// Attempts to upgrade the Weak pointer to an [`Rc`].
    ///
    /// **Note:** Only one thread can have `Rc`s for a value at any point in time.
    /// See [`upgrade()`] to upgrade to an [`Arc`].
    ///
    /// In `no_std` environments this will only succeed if no `Rc` exists on *any* thread.
    ///
    /// # Errors
    /// - [`ValueDropped`]: the referenced value has already been dropped.
    /// - [`WrongThread`]: another thread currently holds `Rc`s for the value.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::{Arc, Rc, Weak, UpgradeError};
    /// # fn main() -> Result<(), UpgradeError> {
    /// let strong = Arc::new(42i32);
    /// let weak = Arc::downgrade(&strong);
    ///
    /// {
    ///     let strong2 = weak.upgrade_local()?;
    ///     assert_eq!(Arc::as_ptr(&strong), Rc::as_ptr(&strong2));
    /// }
    ///
    /// std::mem::drop(strong);
    ///
    /// let error = Weak::upgrade_local(&weak).unwrap_err();
    /// assert_eq!(error, UpgradeError::ValueDropped);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`upgrade()`]: Weak::upgrade
    /// [`ValueDropped`]: UpgradeError::ValueDropped
    /// [`WrongThread`]: UpgradeError::WrongThread
    #[inline]
    pub fn upgrade_local(&self) -> Result<Rc<T>, UpgradeError> {
        let meta = self.meta().ok_or(UpgradeError::ValueDropped)?;
        let current_thread = ThreadId::current_thread();
        
        let owner = match meta.owner.store_if_none(
            Some(current_thread),
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => None,
            Err(owner) => owner,
        };
        
        if owner.is_none() || owner == Some(current_thread) {
            if meta.try_inc_strong_local().is_ok() {
                Ok(HybridRc::<T, Local>::from_inner(self.ptr))
            } else {
                // Relaxed is enough, as try_inc_strong_local failing means that
                // the value was already dropped.
                meta.owner.store(None, Ordering::Relaxed);
                Err(UpgradeError::ValueDropped)
            }
        } else {
            Err(UpgradeError::WrongThread)
        }
    }
    
    /// Attempts to upgrade the Weak pointer to an [`Arc`].
    ///
    /// Also see [`upgrade_local()`] to upgrade to an [`Rc`].
    ///
    /// # Errors
    /// - [`ValueDropped`]: the referenced value has already been dropped.
    ///
    /// [`upgrade_local()`]: Weak::upgrade_local
    /// [`ValueDropped`]: UpgradeError::ValueDropped
    #[inline]
    pub fn upgrade(&self) -> Result<Arc<T>, UpgradeError> {
        let meta = self.meta().ok_or(UpgradeError::ValueDropped)?;
        meta.try_inc_strong_shared()
            .map_err(|_| UpgradeError::ValueDropped)?;
        Ok(HybridRc::<T, Shared>::from_inner(self.ptr))
    }
    
    /// Gets a lower bound to the number of strong pointers to the inner value.
    ///
    /// If `self` was created using [`Weak::new`], this will return 0.
    ///
    /// Please understand that another thread might change the count at any time, including
    /// potentially between calling this method and acting on the result.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use hybrid_rc::{Arc, Rc, Weak};
    ///
    /// let reference = Rc::new(42);
    /// let _2nd_ref = Rc::clone(&reference);
    /// let shared_ref = Rc::to_shared(&reference);
    /// let _2nd_shared_ref = Arc::clone(&shared_ref);
    /// let weak = Rc::downgrade(&reference);
    ///
    /// // shared_ref only knows the count of shared references and that there is at least one
    /// // local reference, so it will show 3 instead of 4:
    /// assert_eq!(Weak::strong_count(&weak), 3);
    /// ```
    #[inline]
    pub fn strong_count(&self) -> usize {
        if let Some(meta) = self.meta() {
            meta.strong_shared.load(Ordering::SeqCst)
        } else {
            0
        }
    }
    
    /// Gets the number of [`Weak`] pointers to this allocation.
    ///
    /// Please understand that another thread may change the count at any time, including
    /// potentially between calling this method and acting on the result. Also there might by
    /// off-by-one errors when other threads concurrently upgrade or downgrade pointers.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use hybrid_rc::{Rc, Weak};
    ///
    /// let reference = Rc::new(42);
    /// let weak = Rc::downgrade(&reference);
    /// let _weak_2 = weak.clone();
    ///
    /// assert_eq!(Weak::weak_count(&weak), 2);
    /// ```
    #[inline]
    pub fn weak_count(&self) -> usize {
        if let Some(meta) = self.meta() {
            let weak = meta.weak.load(Ordering::SeqCst);
            if weak == usize::MAX {
                0
            } else if meta.strong_shared.load(Ordering::SeqCst) > 0 {
                weak - 1
            } else {
                weak
            }
        } else {
            0
        }
    }
}

impl<T> Weak<T> {
    /// Constructs a dummy `Weak<T>`, without allocating any memory.
    ///
    /// Trying to upgrade the result will always result in a [`ValueDropped`] error.
    ///
    /// [`ValueDropped`]: UpgradeError::ValueDropped
    pub fn new() -> Weak<T> {
        Self { ptr: senitel() }
    }
}

impl<T: ?Sized> fmt::Debug for Weak<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(Weak)")
    }
}

impl<T: ?Sized> fmt::Pointer for Weak<T> {
    /// Formats the value using the given formatter.
    ///
    /// If the `#` flag is used, the state (weak) is written after the address.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            fmt::Pointer::fmt(&Self::as_ptr(self), f)?;
            f.write_str(" [weak]")
        } else {
            fmt::Pointer::fmt(&Self::as_ptr(self), f)
        }
    }
}

impl<T> Default for Weak<T> {
    /// Constructs a dummy `Weak<T>`, without allocating any memory.
    ///
    /// See [`Weak<T>::new()`].
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized> Clone for Weak<T> {
    /// Creates another `Weak` reference for the same value.
    ///
    /// # Example
    /// ```ignore
    /// use hybrid_rc::{Rc, Weak};
    ///
    /// let strong = Rc::new(42i32);
    /// let weak = Rc::downgrade(&strong);
    /// let weak2 = Weak::clone(&weak);
    ///
    /// assert_eq!(weak.as_ptr(), weak2.as_ptr());
    /// ```
    #[inline]
    fn clone(&self) -> Self {
        if let Some(meta) = self.meta() {
            // We can ignore the lock in Weak::clone() as the counter is only locked by HybridRc
            // when there are no Weak instances/ (meta.weak == 1).
            meta.inc_weak_nolock();
        }
        Self { ptr: self.ptr }
    }
}

impl<T: ?Sized> Drop for Weak<T> {
    /// Drops the `Weak` reference.
    ///
    /// Once all `HybridRc` and `Weak` references to a shared value are dropped, the shared
    /// allocation is fully released.
    #[inline]
    fn drop(&mut self) {
        if let Some(meta) = self.meta() {
            let last_reference = meta.dec_weak();
            if last_reference {
                unsafe {
                    // Safety: only called if this was the last (weak) reference
                    RcBox::dealloc(self.ptr);
                }
            }
        }
    }
}

// Safety: Like for Arc<T> T: Send + Sync gives all guarantees we need to make Weak Send + Sync.
unsafe impl<T: ?Sized + Sync + Send> Send for Weak<T> {}
unsafe impl<T: ?Sized + Sync + Send> Sync for Weak<T> {}

/// `PinWeak<T>` represents a non-owning reference to a pinned value managed by a
/// [`Pin`]`<`[`HybridRc<T, _>`]`>`.
///
/// The typical way to obtain a `PinWeak<T>` is to call [`HybridRc::downgrade_pin()`].
///
/// See [`Weak<T>`] for more information about weak references.
///
/// [`upgrade()`]: PinWeak::upgrade
/// [`upgrade_local()`]: PinWeak::upgrade_local
#[repr(transparent)]
pub struct PinWeak<T: ?Sized>(Weak<T>);

impl<T: ?Sized> PinWeak<T> {
    /// Attempts to upgrade the pinned weak pointer to a pinned [`Rc`].
    ///
    /// See [`Weak::upgrade_local()`] for more information.
    ///
    /// # Errors
    /// - [`ValueDropped`]: the referenced value has already been dropped.
    /// - [`WrongThread`]: another thread currently holds `Rc`s for the value.
    ///
    /// [`ValueDropped`]: UpgradeError::ValueDropped
    /// [`WrongThread`]: UpgradeError::WrongThread
    #[inline]
    pub fn upgrade_local(&self) -> Result<Pin<Rc<T>>, UpgradeError> {
        Ok(unsafe { Pin::new_unchecked(self.0.upgrade_local()?) })
    }
    
    /// Attempts to upgrade the pinned weak pointer to a pinned [`Arc`].
    ///
    /// See [`Weak::upgrade()`] for more information.
    ///
    /// # Errors
    /// - [`ValueDropped`]: the referenced value has already been dropped.
    ///
    /// [`ValueDropped`]: UpgradeError::ValueDropped
    #[inline]
    pub fn upgrade(&self) -> Result<Pin<Arc<T>>, UpgradeError> {
        Ok(unsafe { Pin::new_unchecked(self.0.upgrade()?) })
    }
    
    /// Gets a lower bound to the number of strong pointers to the inner value.
    ///
    /// See [`Weak::strong_count()`] for more information.
    #[inline]
    pub fn strong_count(&self) -> usize {
        self.0.strong_count()
    }
    
    /// Gets the number of [`Weak`] pointers to this allocation.
    ///
    /// See [`Weak::strong_count()`] for more information.
    #[inline]
    pub fn weak_count(&self) -> usize {
        self.0.weak_count()
    }
    
    /// Transforms this `PinWeak<T>` into a [`Weak<T>`]
    ///
    /// # Safety
    /// This function is unsafe. You must guarantee that you will continue to treat the `Weak` as
    /// pinned after you call this function. Not maintaining the pinning invariants that is a
    /// violation of the API contract and may lead to undefined behavior in later (safe) operations.
    ///
    /// If the underlying data is [`Unpin`], [`PinWeak::into_inner()`] should be used instead.
    #[inline]
    pub unsafe fn into_inner_unchecked(self) -> Weak<T> {
        self.0
    }
}

impl<T> PinWeak<T> {
    /// Constructs a dummy `PinWeak<T>`, without allocating any memory.
    ///
    /// Trying to upgrade the result will always result in a [`ValueDropped`] error.
    ///
    /// [`ValueDropped`]: UpgradeError::ValueDropped
    pub fn new() -> PinWeak<T> {
        Self(Weak::new())
    }
}

impl<T: ?Sized + Unpin> PinWeak<T> {
    /// Transforms this `PinWeak<T>` into a [`Weak<T>`]
    ///
    /// This requires that the data inside the shared allocation is [`Unpin`], so that we
    /// can ignore the pinning invariants when unwrapping it.
    #[inline]
    pub fn into_inner(self) -> Weak<T> {
        self.0
    }
}

impl<T: ?Sized> fmt::Debug for PinWeak<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pin<(Weak)>")
    }
}

impl<T: ?Sized> fmt::Pointer for PinWeak<T> {
    /// Formats the value using the given formatter.
    ///
    /// If the `#` flag is used, the state (weak) is written after the address.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            fmt::Pointer::fmt(&self.0.as_ptr(), f)?;
            f.write_str(" [weak]")
        } else {
            fmt::Pointer::fmt(&self.0.as_ptr(), f)
        }
    }
}

impl<T: ?Sized> Clone for PinWeak<T> {
    /// Creates another pinned weak reference for the same value.
    ///
    /// See [`Weak::clone()`] for more information.
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for PinWeak<T> {
    /// Constructs a dummy `PinWeak<T>`, without allocating any memory.
    ///
    /// See [`PinWeak<T>::new()`].
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// Safety: Like for Weak<T> T: Send + Sync gives all guarantees we need to make PinWeak Send + Sync.
unsafe impl<T: ?Sized + Sync + Send> Send for PinWeak<T> {}
unsafe impl<T: ?Sized + Sync + Send> Sync for PinWeak<T> {}
