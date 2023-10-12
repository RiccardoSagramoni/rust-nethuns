use core::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::process::abort;
use std::ptr;
use std::ptr::NonNull;
use std::sync::atomic;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};


/// A soft limit on the amount of references that may be made to a `SendRc`.
///
/// Going above this limit will abort your program (although not
/// necessarily) at _exactly_ `MAX_REFCOUNT + 1` references.
/// Trying to go above it might call a `panic` (if not actually going above it).
///
/// This is a global invariant, and also applies when using a compare-exchange loop.
///
/// See comment in `SendRc::clone`.
const MAX_REFCOUNT: usize = (isize::MAX) as usize;


/// A thread-safe reference-counting pointer,
/// which only implements the [`Send`] trait.
/// 
/// This is a modified version of [`Arc`](std::sync::Arc) with only the 
/// necessary methods for the Nethuns library and without the [`Sync`] requirement.
///
/// The type `SendRc<T>` provides shared ownership of a value of type `T`,
/// allocated in the heap. Invoking [`clone`][clone] on `SendRc` produces
/// a new `SendRc` instance, which points to the same allocation on the heap as the
/// source `SendRc`, while increasing a reference count. When the last `SendRc`
/// pointer to a given allocation is destroyed, the value stored in that allocation (often
/// referred to as "inner value") is also dropped.
///
/// Shared references in Rust disallow mutation by default, and `SendRc` is no
/// exception: you cannot generally obtain a mutable reference to something
/// inside an `SendRc`. If you need to mutate through an `SendRc`, use
/// [`Mutex`][std::sync::Mutex], [`RwLock`][std::sync::RwLock], or one of the [`Atomic`][atomic]
/// types.
///
/// **Note**: This type is only available on platforms that support atomic
/// loads and stores of pointers, which includes all platforms that support
/// the `std` crate but not all those which only support [`alloc`](crate).
/// This may be detected at compile time using `#[cfg(target_has_atomic = "ptr")]`.
///
/// ## Thread Safety
///
/// Unlike [`Rc<T>`](std::rc::Rc), `SendRc<T>` uses atomic operations for its reference
/// counting. This means that it is thread-safe. The disadvantage is that
/// atomic operations are more expensive than ordinary memory accesses. If you
/// are not sharing reference-counted allocations between threads, consider using
/// [`Rc<T>`](std::rc::Rc) for lower overhead. [`Rc<T>`](std::rc::Rc) is a safe default, because the
/// compiler will catch any attempt to send an [`Rc<T>`](std::rc::Rc) between threads.
/// However, a library might choose `SendRc<T>` in order to give library consumers
/// more flexibility.
///
/// `SendRc<T>` will implement [`Send`] as long as the `T` implements
/// [`Send`]. It doesn't implement [`Sync`] in any case (see [`Arc<T>`](std::sync::Arc), if you need an alternative which implements both [`Sync`] and [`Send`]).
/// This means that you may have to pair `SendRc<T>` with some sort of
/// [`std::sync`](std::sync) type, usually [`Mutex<T>`][std::sync::Mutex],
/// if you need to synchronize access to the inner value.
///
///
/// # Cloning references
///
/// Creating a new reference from an existing reference-counted pointer is done using the
/// `Clone` trait implemented for [`SendRc<T>`][SendRc].
///
/// ```ignore
/// let foo = SendRc::new(vec![1.0, 2.0, 3.0]);
/// // The two syntaxes below are equivalent.
/// let a = foo.clone();
/// let b = SendRc::clone(&foo);
/// // a, b, and foo are all SendRcs that point to the same memory location
/// ```
///
/// ## `Deref` behavior
///
/// `SendRc<T>` automatically dereferences to `T` (via the [`Deref`] trait),
/// so you can call `T`'s methods on a value of type `SendRc<T>`. To avoid name
/// clashes with `T`'s methods, the methods of `SendRc<T>` itself are associated
/// functions, called using [fully qualified syntax]:
///
/// ```ignore
/// let my_arc = SendRc::new(());
/// let my_weak = SendRc::downgrade(&my_arc);
/// ```
///
/// `SendRc<T>`'s implementations of traits like `Clone` may also be called using
/// fully qualified syntax. Some people prefer to use fully qualified syntax,
/// while others prefer using method-call syntax.
///
/// ```ignore
/// let arc = SendRc::new(());
/// // Method-call syntax
/// let arc2 = arc.clone();
/// // Fully qualified syntax
/// let arc3 = SendRc::clone(&arc);
/// ```
///
///
/// # Examples
///
/// Sharing some immutable data between threads:
///
// Note that we **do not** run these tests here. The windows builders get super
// unhappy if a thread outlives the main thread and then exits at the same time
// (something deadlocks) so we just avoid this entirely by not running these
// tests.
/// ```ignore
/// use std::thread;
/// use super::SendRc;
///
/// let five = SendRc::new(5);
///
/// for _ in 0..10 {
///     let five = SendRc::clone(&five);
///
///     thread::spawn(move || {
///         println!("{five:?}");
///     });
/// }
/// ```
///
/// Sharing a mutable [`AtomicUsize`]:
///
/// [`AtomicUsize`]: core::sync::atomic::AtomicUsize "sync::atomic::AtomicUsize"
///
/// ```ignore
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use std::thread;
///
/// let val = SendRc::new(AtomicUsize::new(5));
///
/// for _ in 0..10 {
///     let val = SendRc::clone(&val);
///
///     thread::spawn(move || {
///         let v = val.fetch_add(1, Ordering::SeqCst);
///         println!("{v:?}");
///     });
/// }
/// ```
pub struct SendRc<T: ?Sized> {
    ptr: NonNull<SendRcInner<T>>,
    phantom: PhantomData<SendRcInner<T>>,
}

unsafe impl<T: ?Sized + Send> Send for SendRc<T> {}
impl<T: RefUnwindSafe + ?Sized> UnwindSafe for SendRc<T> {}


// This is repr(C) to future-proof against possible field-reordering, which
// would interfere with otherwise safe [into|from]_raw() of transmutable
// inner types.
#[repr(C)]
struct SendRcInner<T: ?Sized> {
    rc: atomic::AtomicUsize,
    data: T,
}

unsafe impl<T: ?Sized + Send> Send for SendRcInner<T> {}


impl<T: ?Sized> SendRc<T> {
    unsafe fn from_inner(ptr: NonNull<SendRcInner<T>>) -> Self {
        Self {
            ptr,
            phantom: PhantomData,
        }
    }
}


impl<T> SendRc<T> {
    /// Constructs a new `SendRc<T>`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let five = SendRc::new(5);
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[inline]
    pub fn new(data: T) -> SendRc<T> {
        // Start the weak pointer count as 1 which is the weak pointer that's
        // held by all the strong pointers (kinda), see std/rc.rs for more info
        let x: Box<_> = Box::new(SendRcInner {
            rc: atomic::AtomicUsize::new(1),
            data,
        });
        unsafe { Self::from_inner(Box::leak(x).into()) }
    }
}

impl<T: ?Sized> SendRc<T> {
    /// Gets the number of strong (`SendRc`) pointers to this allocation.
    ///
    /// # Safety
    ///
    /// This method by itself is safe, but using it correctly requires extra care.
    /// Another thread can change the strong count at any time,
    /// including potentially between calling this method and acting on the result.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let five = SendRc::new(5);
    /// let _also_five = SendRc::clone(&five);
    ///
    /// // This assertion is deterministic because we haven't shared
    /// // the `SendRc` between threads.
    /// assert_eq!(2, SendRc::strong_count(&five));
    /// ```
    #[inline]
    #[must_use]
    pub fn strong_count(this: &Self) -> usize {
        this.inner().rc.load(Acquire)
    }
    
    #[inline]
    fn inner(&self) -> &SendRcInner<T> {
        // This unsafety is ok because while this arc is alive we're guaranteed
        // that the inner pointer is valid. Furthermore, we know that the
        // `SendRcInner` structure itself is `Sync` because the inner data is
        // `Sync` as well, so we're ok loaning out an immutable pointer to these
        // contents.
        unsafe { self.ptr.as_ref() }
    }
    
    /// Returns `true` if the two `SendRc`s point to the same allocation in a vein similar to
    /// [`ptr::eq`].
    ///
    /// Note that comparing trait object pointers (*const dyn Trait) is unreliable: pointers to values of the same underlying type can compare inequal (because vtables are duplicated in multiple codegen units), and pointers to values of different underlying type can compare equal (since identical vtables can be deduplicated within a codegen unit).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let five = SendRc::new(5);
    /// let same_five = SendRc::clone(&five);
    /// let other_five = SendRc::new(5);
    ///
    /// assert!(SendRc::ptr_eq(&five, &same_five));
    /// assert!(!SendRc::ptr_eq(&five, &other_five));
    /// ```
    ///
    /// [`ptr::eq`]: core::ptr::eq "ptr::eq"
    #[inline]
    #[must_use]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        ptr::eq(this.ptr.as_ptr(), other.ptr.as_ptr())
    }
}


impl<T: ?Sized> Clone for SendRc<T> {
    /// Makes a clone of the `SendRc` pointer.
    ///
    /// This creates another pointer to the same allocation, increasing the
    /// strong reference count.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let five = SendRc::new(5);
    ///
    /// let _ = SendRc::clone(&five);
    /// ```
    #[inline]
    fn clone(&self) -> SendRc<T> {
        // Using a relaxed ordering is alright here, as knowledge of the
        // original reference prevents other threads from erroneously deleting
        // the object.
        //
        // As explained in the [Boost documentation][1], Increasing the
        // reference counter can always be done with memory_order_relaxed: New
        // references to an object can only be formed from an existing
        // reference, and passing an existing reference from one thread to
        // another must already provide any required synchronization.
        //
        // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
        let old_size = self.inner().rc.fetch_add(1, Relaxed);
        
        // However we need to guard against massive refcounts in case someone is `mem::forget`ing
        // SendRcs. If we don't do this the count can overflow and users will use-after free. This
        // branch will never be taken in any realistic program. We abort because such a program is
        // incredibly degenerate, and we don't care to support it.
        //
        // This check is not 100% water-proof: we error when the refcount grows beyond `isize::MAX`.
        // But we do that check *after* having done the increment, so there is a chance here that
        // the worst already happened and we actually do overflow the `usize` counter. However, that
        // requires the counter to grow from `isize::MAX` to `usize::MAX` between the increment
        // above and the `abort` below, which seems exceedingly unlikely.
        //
        // This is a global invariant, and also applies when using a compare-exchange loop to increment
        // counters in other methods.
        // Otherwise, the counter could be brought to an almost-overflow using a compare-exchange loop,
        // and then overflow using a few `fetch_add`s.
        if old_size > MAX_REFCOUNT {
            abort();
        }
        
        unsafe { Self::from_inner(self.ptr) }
    }
}


impl<T: ?Sized + fmt::Display> fmt::Display for SendRc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}


impl<T: ?Sized + fmt::Debug> fmt::Debug for SendRc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}


impl<T: Default> Default for SendRc<T> {
    /// Creates a new `SendRc<T>`, with the `Default` value for `T`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let x: SendRc<i32> = Default::default();
    /// assert_eq!(*x, 0);
    /// ```
    fn default() -> SendRc<T> {
        SendRc::new(Default::default())
    }
}


impl<T: ?Sized> Deref for SendRc<T> {
    type Target = T;
    
    #[inline]
    fn deref(&self) -> &T {
        &self.inner().data
    }
}


impl<T: ?Sized> SendRc<T> {
    /// Returns a mutable reference into the given `SendRc`, if there are
    /// no other `SendRc` or [`Weak`] pointers to the same allocation.
    ///
    /// Returns [`None`] otherwise, because it is not safe to
    /// mutate a shared value.
    ///
    /// See also [`make_mut`][make_mut], which will [`clone`][clone]
    /// the inner value when there are other `SendRc` pointers.
    ///
    /// [make_mut]: SendRc::make_mut
    /// [clone]: Clone::clone
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut x = SendRc::new(3);
    /// *SendRc::get_mut(&mut x).unwrap() = 4;
    /// assert_eq!(*x, 4);
    ///
    /// let _y = SendRc::clone(&x);
    /// assert!(SendRc::get_mut(&mut x).is_none());
    /// ```
    #[inline]
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        if this.is_unique() {
            // This unsafety is ok because we're guaranteed that the pointer
            // returned is the *only* pointer that will ever be returned to T. Our
            // reference count is guaranteed to be 1 at this point, and we required
            // the SendRc itself to be `mut`, so we're returning the only possible
            // reference to the inner data.
            unsafe { Some(SendRc::get_mut_unchecked(this)) }
        } else {
            None
        }
    }
    
    /// Returns a mutable reference into the given `SendRc`,
    /// without any check.
    ///
    /// See also [`get_mut`], which is safe and does appropriate checks.
    ///
    /// [`get_mut`]: SendRc::get_mut
    ///
    /// # Safety
    ///
    /// If any other `SendRc` or [`Weak`] pointers to the same allocation exist, then
    /// they must not be dereferenced or have active borrows for the duration
    /// of the returned borrow, and their inner type must be exactly the same as the
    /// inner type of this Rc (including lifetimes). This is trivially the case if no
    /// such pointers exist, for example immediately after `SendRc::new`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut x = SendRc::new(String::new());
    /// unsafe {
    ///     SendRc::get_mut_unchecked(&mut x).push_str("foo")
    /// }
    /// assert_eq!(*x, "foo");
    /// ```
    /// Other `SendRc` pointers to the same allocation must be to the same type.
    /// ```ignore
    /// let x: SendRc<str> = SendRc::from("Hello, world!");
    /// let mut y: SendRc<[u8]> = x.clone().into();
    /// unsafe {
    ///     // this is Undefined Behavior, because x's inner type is str, not [u8]
    ///     SendRc::get_mut_unchecked(&mut y).fill(0xff); // 0xff is invalid in UTF-8
    /// }
    /// println!("{}", &*x); // Invalid UTF-8 in a str
    /// ```
    /// Other `SendRc` pointers to the same allocation must be to the exact same type, including lifetimes.
    /// ```ignore
    /// let x: SendRc<&str> = SendRc::new("Hello, world!");
    /// {
    ///     let s = String::from("Oh, no!");
    ///     let mut y: SendRc<&str> = x.clone().into();
    ///     unsafe {
    ///         // this is Undefined Behavior, because x's inner type
    ///         // is &'long str, not &'short str
    ///         *SendRc::get_mut_unchecked(&mut y) = &s;
    ///     }
    /// }
    /// println!("{}", &*x); // Use-after-free
    /// ```
    #[inline]
    pub unsafe fn get_mut_unchecked(this: &mut Self) -> &mut T {
        // We are careful to *not* create a reference covering the "count" fields, as
        // this would alias with concurrent access to the reference counts (e.g. by `Weak`).
        unsafe { &mut (*this.ptr.as_ptr()).data }
    }
    
    /// Determine whether this is the unique reference (including weak refs) to
    /// the underlying data.
    ///
    /// Note that this requires locking the weak ref count.
    fn is_unique(&mut self) -> bool {
        // This needs to be an `Acquire` to synchronize with the decrement of the `strong`
        // counter in `drop` -- the only access that happens when any but the last reference
        // is being dropped.
        self.inner().rc.load(Acquire) == 1
    }
}

impl<T: ?Sized> Drop for SendRc<T> {
    /// Drops the `SendRc`.
    ///
    /// This will decrement the strong reference count. If the strong reference
    /// count reaches zero then the only other references (if any) are
    /// [`Weak`], so we `drop` the inner value.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// struct Foo;
    ///
    /// impl Drop for Foo {
    ///     fn drop(&mut self) {
    ///         println!("dropped!");
    ///     }
    /// }
    ///
    /// let foo  = SendRc::new(Foo);
    /// let foo2 = SendRc::clone(&foo);
    ///
    /// drop(foo);    // Doesn't print anything
    /// drop(foo2);   // Prints "dropped!"
    /// ```
    #[inline]
    fn drop(&mut self) {
        // Because `fetch_sub` is already atomic, we do not need to synchronize
        // with other threads unless we are going to delete the object. This
        // same logic applies to the below `fetch_sub` to the `weak` count.
        if self.inner().rc.fetch_sub(1, Release) != 1 {
            return;
        }
        
        // This fence is needed to prevent reordering of use of the data and
        // deletion of the data. Because it is marked `Release`, the decreasing
        // of the reference count synchronizes with this `Acquire` fence. This
        // means that use of the data happens before decreasing the reference
        // count, which happens before this fence, which happens before the
        // deletion of the data.
        //
        // As explained in the [Boost documentation][1],
        //
        // > It is important to enforce any possible access to the object in one
        // > thread (through an existing reference) to *happen before* deleting
        // > the object in a different thread. This is achieved by a "release"
        // > operation after dropping a reference (any access to the object
        // > through this reference must obviously happened before), and an
        // > "acquire" operation before deleting the object.
        //
        // In particular, while the contents of an SendRc are usually immutable, it's
        // possible to have interior writes to something like a Mutex<T>. Since a
        // Mutex is not acquired when it is deleted, we can't rely on its
        // synchronization logic to make writes in thread A visible to a destructor
        // running in thread B.
        //
        // Also note that the Acquire fence here could probably be replaced with an
        // Acquire load, which could improve performance in highly-contended
        // situations. See [2].
        //
        // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
        // [2]: (https://github.com/rust-lang/rust/pull/41714)
        atomic::fence(Acquire);
        
        // We use Box::from_raw to drop the boxed SendRcInner<T> and its data.
        // This is safe as we know we have the last pointer to the `SendRcInner`
        // and that its pointer is valid.
        unsafe {
            drop(Box::from_raw(self.ptr.as_ptr()));
        }
    }
}
