#![cfg(test)]

//! Tests for the main library module

extern crate std;
use super::*;
use alloc::{format, vec};
use core::convert::TryInto;
use core::fmt::Debug;
use std::collections::hash_map;
use std::{thread, thread_local, cmp};

thread_local! {
    static DROP_COUNTER: Cell<usize> = Cell::new(0);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Test(u8);
impl Drop for Test {
    fn drop(&mut self) {
        DROP_COUNTER.with(|x| x.set(x.get() + 1));
    }
}
impl Default for Test {
    fn default() -> Self {
        Test(1)
    }
}
// Panicing clone to test cloning from slice error cases
impl Clone for Test {
    fn clone(&self) -> Self {
        if self.0 == 0 {
            panic!();
        }
        Self(self.0)
    }
}

#[test]
fn test_traits() {
    // Default
    let a = Rc::<Test>::default();
    let b = Arc::<Test>::default();
    
    // From<T>
    let _: Rc<u64> = 1337u64.into();
    let _: Arc<u64> = 1337u64.into();
    
    // PartialEq
    assert_eq!(a, b);
    
    // PartialOrd
    assert!(a <= b && a >= b && !(a < b) && !(a > b));
    
    // Ord
    assert_eq!(a.cmp(&a), cmp::Ordering::Equal);
    
    // Hash
    let mut map = hash_map::HashMap::new();
    map.insert(a.clone(), ());
    assert!(map.get(&a).is_some());
    
    // Deref, Borrow, AsRef
    assert_eq!(&*a, a.borrow());
    assert_eq!(a.deref(), a.as_ref());
}

#[test]
fn test_into_slice() {
    let a = Rc::<[usize; 4]>::new([0, 1, 2, 3]);
    let b: Rc<[usize]> = From::from(a);
    assert_eq!(b.len(), 4);
    for (i, n) in (*b).iter().enumerate() {
        assert_eq!(i, *n);
    }
}

#[test]
fn test_any() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Rc::<Test>::new(Test(42));
    let b: Rc<dyn Any> = From::from(a.clone());
    let c: Rc<dyn Any + Send + Sync> = From::from(a);
    
    assert!(b.clone().downcast::<usize>().is_err());
    assert!(c.clone().downcast::<usize>().is_err());
    let a = c.downcast::<Test>().unwrap();
    assert_eq!(a.0, 42);
    let b = b.downcast::<Test>().unwrap();
    assert_eq!(b.0, 42);
}

#[test]
fn test_pin() {
    let a = Rc::<usize>::pin(42);
    let b = Rc::to_shared_pin(&a);
    assert!(Rc::ptr_eq_pin(&a, &b));
    
    let weak = Arc::downgrade_pin(&b);
    let c = weak.upgrade().unwrap();
    assert!(Rc::ptr_eq_pin(&a, &c));
    
    let d: Rc<usize> = Pin::into_inner(a.clone());
    assert_eq!(
        Pin::into_inner(a.as_ref()) as *const _,
        Rc::as_ref(&d) as *const _
    );
}

#[test]
fn test_local() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Rc::new(Test::default());
    let b = a.clone();
    mem::drop(a);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    mem::drop(b);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
}

#[test]
fn test_shared() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Arc::new(Test::default());
    let b = a.clone();
    mem::drop(a);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    mem::drop(b);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
}

#[test]
fn test_shared_to_local() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Arc::new(Test::default());
    let b = Arc::to_local(&a).unwrap();
    mem::drop(a);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    mem::drop(b);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
}

#[test]
fn test_shared_to_local_2() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Arc::new(Test::default());
    let b = Arc::to_local(&a).unwrap();
    // Create a second Rc from the Arc on the same thread
    // Only possible with thread identification -> `std` needed
    let _: Rc<Test> = TryFrom::try_from(a.clone()).unwrap();
    mem::drop(a);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    mem::drop(b);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
}

#[test]
fn test_shared_to_local_on_wrong_thread() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Arc::new(Test::default());
    let b = Arc::to_local(&a).unwrap();
    assert!(thread::spawn(move || Arc::to_local(&a).is_none()
        && TryInto::<Rc<Test>>::try_into(a).is_err())
    .join()
    .unwrap());
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    mem::drop(b);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
}

#[test]
fn test_local_to_shared() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Rc::new(Test::default());
    let b = Rc::to_shared(&a);
    mem::drop(a);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    mem::drop(b);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
}

#[test]
fn test_get_mut() {
    let mut a = Rc::new(Test::default());
    assert_eq!(
        unsafe { Rc::get_mut_unchecked(&mut a) } as *const Test,
        Rc::as_ptr(&a)
    );
    assert!(Rc::get_mut(&mut a).is_some());
    let mut b = Rc::clone(&a);
    assert!(Rc::get_mut(&mut a).is_none());
    mem::drop(a);
    assert!(Rc::get_mut(&mut b).is_some());
    let w = Rc::downgrade(&b);
    assert!(Rc::get_mut(&mut b).is_none());
    mem::drop(w);
}

#[test]
fn test_dangling_weak() {
    DROP_COUNTER.with(|x| x.set(0));
    let w = Weak::<Test>::new();
    assert!(w.upgrade_local().is_err());
    assert!(w.upgrade().is_err());
    assert_eq!(w.strong_count(), 0);
    assert_eq!(w.weak_count(), 0);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
}

#[test]
fn test_weak() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Rc::new(Test::default());
    let a_ptr = Rc::as_ptr(&a);
    
    let w = Rc::downgrade(&a);
    assert!(thread::spawn(move || w.upgrade_local().is_err())
        .join()
        .unwrap());
    let w = Rc::downgrade(&a);
    assert!(thread::spawn(move || w.upgrade().is_ok()).join().unwrap());
    
    let b = Rc::to_shared(&a);
    mem::drop(a);
    let w = Arc::downgrade(&b);
    let _ = w.clone();
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    assert!(w.upgrade_local().is_ok());
    mem::drop(b);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
    assert!(w.upgrade_local().is_err());
    assert!(w.upgrade().is_err());
    assert_eq!(w.as_ptr(), a_ptr);
    assert!(!is_senitel(w.as_ptr()));
}

#[test]
fn test_weak_upgrade_local() {
    DROP_COUNTER.with(|x| x.set(0));
    let a = Rc::new(Test::default());
    let w = Rc::downgrade(&a);
    let b = Rc::to_shared(&a);
    
    // Upgrade to Rc while a Rc already exists. Only possible with thread identification
    // -> `std` needed
    assert!(w.upgrade_local().is_ok());
    mem::drop(a);
    assert!(w.upgrade_local().is_ok());
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    mem::drop(b);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
    assert!(w.upgrade_local().is_err());
}

#[test]
fn test_counters() {
    assert_eq!(Weak::<()>::default().weak_count(), 0);
    let a = Rc::new(());
    assert_eq!(Rc::strong_count(&a), 1);
    assert_eq!(Rc::weak_count(&a), 0);
    let b = Rc::clone(&a);
    assert_eq!(Rc::strong_count(&a), 2);
    assert_eq!(Rc::strong_count(&b), 2);
    assert_eq!(Rc::weak_count(&a), 0);
    let c = Rc::to_shared(&a);
    assert_eq!(Rc::strong_count(&a), 3);
    assert_eq!(Arc::strong_count(&c), 2);
    assert_eq!(Rc::weak_count(&a), 0);
    let w = Rc::downgrade(&a);
    assert_eq!(Rc::strong_count(&a), 3);
    assert_eq!(Weak::strong_count(&w), 2);
    assert_eq!(Weak::weak_count(&w), 1);
    let w2 = w.clone();
    assert_eq!(Weak::strong_count(&w), 2);
    assert_eq!(Weak::weak_count(&w), 2);
    mem::drop(a);
    assert_eq!(Weak::strong_count(&w), 2);
    assert_eq!(Weak::weak_count(&w), 2);
    mem::drop(c);
    assert_eq!(Weak::strong_count(&w), 1);
    assert_eq!(Weak::weak_count(&w), 2);
    mem::drop(b);
    assert_eq!(Weak::strong_count(&w), 0);
    assert_eq!(Weak::weak_count(&w), 2);
    mem::drop(w2);
    assert_eq!(Weak::weak_count(&w), 1);
}

#[test]
fn test_try_unwrap() {
    let a = Rc::new(());
    let b = Rc::clone(&a);
    let c = Rc::to_shared(&a);
    let w = Rc::downgrade(&a);
    
    let a = Rc::try_unwrap(a).unwrap_err();
    mem::drop(b);
    let a = Rc::try_unwrap(a).unwrap_err();
    let c = Arc::try_unwrap(c).unwrap_err();
    mem::drop(c);
    assert!(w.upgrade().is_ok());
    let _value = Rc::try_unwrap(a).unwrap();
    assert!(w.upgrade().is_err());
}

#[test]
fn test_fmt() {
    let a = Rc::new(String::from("abc"));
    let _ = format!("{0:?} {0:p} {0}", a);
    assert!(format!("{0:#p}", a).find("local").is_some());
    let b = Rc::to_shared(&a);
    assert!(format!("{0:#p}", b).find("shared").is_some());
    let c = Rc::downgrade(&a);
    assert!(format!("{0:p}", c).find("weak").is_none());
    assert!(format!("{0:?}", c).find("(Weak)").is_some());
    assert!(format!("{0:#p}", c).find("weak").is_some());
    
    let _ = format!("{0:?} {0}", UpgradeError::ValueDropped);
    let _ = format!("{0:?} {0}", UpgradeError::WrongThread);
    let _ = format!("{0:?} {0}", AllocError);
}

#[test]
fn test_error_traits() {
    // Test copy, clone and equality of AllocError
    let a = AllocError;
    let b = a;
    assert_eq!(a, b);
    assert_eq!(a.clone(), b);
    
    // Test copy, clone and equality of UpgradeError
    let a = UpgradeError::ValueDropped;
    let b = UpgradeError::WrongThread;
    let c = a;
    assert_ne!(a, b);
    assert_eq!(a, c);
    assert_eq!(b, b.clone());
}

#[test]
fn test_senitel() {
    let x = senitel::<()>();
    assert!(is_senitel(x.as_ptr()));
    let w = Weak::<()>::default();
    assert!(is_senitel(w.as_ptr()));
}

#[test]
fn test_ptr_eq() {
    let x = Rc::new(());
    let x2 = Rc::to_shared(&x);
    assert!(Rc::ptr_eq(&x, &x2));
    let y = Rc::new(());
    assert!(!Rc::ptr_eq(&x, &y));
}

#[test]
fn test_uninit() {
    let mut a = Rc::<u64>::new_uninit();
    Rc::get_mut(&mut a).unwrap().write(42);
    let a = unsafe { a.assume_init() };
    assert_eq!(*a, 42);
    
    // Uninit without `assume_init()` should never drop the inner value
    DROP_COUNTER.with(|x| x.set(0));
    let _ = Rc::<Test>::new_uninit();
    let _ = Rc::<[Test]>::new_uninit_slice(3);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
}

#[test]
fn test_zeroed() {
    let a = Rc::<u64>::new_zeroed();
    // Zeroed memory is a valid u64 value, so this is safe
    let a = unsafe { a.assume_init() };
    assert_eq!(*a, 0);
}

#[test]
fn test_make_mut() {
    let mut a = Rc::new(42);
    *Rc::make_mut(&mut a) /= 2;
    assert_eq!(*a, 21);
    
    let weak = Rc::downgrade(&a);
    assert!(weak.upgrade().is_ok());
    *Rc::make_mut(&mut a) += 2;
    assert!(weak.upgrade().is_err());
    
    let mut b: Arc<i32> = From::from(a.clone());
    *Arc::make_mut(&mut b) *= 2;
    assert_eq!(*a, 23);
    assert_eq!(*b, 46);
}

#[test]
fn test_slice_into_array() {
    let a = unsafe { Rc::<[u8]>::new_zeroed_slice(42).assume_init() };
    let a = TryInto::<Rc<[u8; 32]>>::try_into(a).unwrap_err();
    let a = TryInto::<Rc<[u8; 0]>>::try_into(a).unwrap_err();
    let _ = TryInto::<Rc<[u8; 42]>>::try_into(a).unwrap();
}

/// Briefly test the `try_` variants behave like their counterparts
#[test]
fn test_try_new() {
    let _ = *Rc::try_new(42).unwrap() == *Rc::new(42);
    assert_eq!(
        *unsafe { Rc::<u32>::try_new_zeroed().unwrap().assume_init() },
        0
    );
    let _ = Rc::<u32>::try_new_uninit().unwrap();
}

#[test]
fn test_str() {
    let a: Rc<str> = Rc::from("test");
    let b: Rc<str> = Rc::from("test".to_owned());
    assert_eq!(a[..], b[..]);
    assert_eq!(&a[..], "test");
}

#[test]
fn test_cow_str() {
    let cow = Cow::Borrowed("test");
    let cow_owned = Cow::Owned("test".to_owned());
    let a: Rc<str> = Rc::from(cow);
    let b: Rc<str> = Rc::from(cow_owned);
    assert_eq!(a[..], b[..]);
    assert_eq!(&a[..], "test");
}

#[test]
fn test_from_slice() {
    let array = ["a".to_owned(), "b".to_owned(), "c".to_owned()];
    let a: Rc<[_]> = (&array[..]).into();
    assert_eq!(&a[..], &array);
    
    DROP_COUNTER.with(|x| x.set(0));
    let array = [Test(1), Test(2), Test(3), Test(0), Test(4)];
    let result = std::panic::catch_unwind(|| {
        let _: Rc<[_]> = array[..].into();
    });
    assert!(result.is_err());
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 3);
    
    let array = [1, 2, 3];
    let a = Rc::copy_from_slice(&array);
    assert_eq!(&a[..], &array);
}

#[test]
fn test_from_vec() {
    let vec = vec!["a", "b", "c"];
    let a: Rc<[_]> = vec.into();
    assert_eq!(&a[..], &["a", "b", "c"]);
    
    DROP_COUNTER.with(|x| x.set(0));
    let vec = vec![Test(1), Test(2), Test(3), Test(0), Test(4)];
    let rc: Rc<[_]> = vec.into();
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    drop(rc);
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 5);
}

#[test]
fn test_collect() {
    let vec = vec![1, 2, 3, 4];
    let a: Rc<[i32]> = vec.into_iter().collect();
    assert_eq!(&a[..], &[1, 2, 3, 4]);
}

#[test]
fn test_from_box() {
    let b: Box<i64> = Box::new(42);
    let rc: Rc<i64> = b.into();
    assert_eq!(*rc, 42);
}

#[test]
fn test_from_box_slice() {
    let vec = vec![1, 2, 3, 4];
    let b: Box<[i32]> = Box::from(&vec[..]);
    let rc: Rc<[i32]> = From::<Box<_>>::from(b);
    assert_eq!(&rc[..], &vec[..]);
}

#[test]
fn test_from_box_trait_obj() {
    let map = hash_map::HashMap::<i32, i64>::new();
    
    let original_box = Box::new(map.clone());
    let erased_box: Box<dyn Debug> = original_box;
    let rc: Rc<dyn Debug> = From::<Box<_>>::from(erased_box);
    assert_eq!(format!("{:?}", &*rc), format!("{:?}", &map));
}

#[test]
fn test_raw_conversion() {
    let rc: Rc<i64> = Rc::new(1337);
    let ptr = Rc::into_raw(rc);
    
    let rc: Rc<i64> = unsafe { Rc::from_raw(ptr) };
    assert_eq!(*rc, 1337);
    
    // ---
    
    let arc: Arc<str> = Arc::from("hallo");
    let ptr = Arc::into_raw(arc);
    
    let arc: Arc<str> = unsafe { Arc::from_raw(ptr) };
    assert_eq!(&*arc, "hallo");
}

#[test]
fn test_raw_counter_manipulation() {
    let rc: Rc<i64> = Rc::new(1337);
    let weak = Rc::downgrade(&rc);
    let ptr = Rc::into_raw(rc);
    
    unsafe {
        Rc::increment_local_strong_count(ptr);
        assert!(weak.upgrade().is_ok());
        Rc::decrement_local_strong_count(ptr);
        assert!(weak.upgrade().is_ok());
        Rc::decrement_local_strong_count(ptr);
        assert!(weak.upgrade().is_err());
    }
    
    // ---
    
    let arc: Arc<i64> = Arc::new(42);
    let weak = Arc::downgrade(&arc);
    let ptr = Arc::into_raw(arc);
    
    unsafe {
        Arc::increment_shared_strong_count(ptr);
        assert!(weak.upgrade().is_ok());
        Arc::decrement_shared_strong_count(ptr);
        assert!(weak.upgrade().is_ok());
        Arc::decrement_shared_strong_count(ptr);
        assert!(weak.upgrade().is_err());
    }
}


#[test]
fn test_cyclic() {
    struct Cycle(Weak<Cycle>, Test);
    
    DROP_COUNTER.with(|x| x.set(0));
    
    {
        let rc = Rc::new_cyclic(|w| {
            assert!(w.upgrade().is_err());
            Cycle(w.clone(), Test(1))
        });
        let weak = &(*rc).0;
        let arc = weak.upgrade().unwrap();
        assert!(Rc::ptr_eq(&rc, &arc));
        assert_eq!(rc.1 .0, 1);
        assert_eq!(&(*rc).0 as *const _, weak as *const _);
        assert_eq!(DROP_COUNTER.with(|x| x.get()), 0);
    }
    
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 1);
    
    {
        let arc = Arc::new_cyclic(|w| {
            assert!(w.upgrade().is_err());
            Cycle(w.clone(), Test(2))
        });
        let weak = &(*arc).0;
        let arc2 = weak.upgrade().unwrap();
        assert!(Arc::ptr_eq(&arc, &arc2));
        assert_eq!(arc.1 .0, 2);
        assert_eq!(&(*arc2).0 as *const _, weak as *const _);
    }
    
    assert_eq!(DROP_COUNTER.with(|x| x.get()), 2);
}


#[test]
#[ignore = "takes a long time in debug build"]
#[should_panic]
fn test_local_counter_overflow() {
    let rc: Rc<i64> = Rc::new(1337);
    let ptr = Rc::into_raw(rc);
    for _ in 0..=usize::MAX {
        unsafe {
            Rc::increment_local_strong_count(ptr);
        }
    }
}
