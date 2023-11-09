# hybrid-rc - Thread-safe hybrid reference counting pointers

[![Crates.io](https://img.shields.io/crates/v/hybrid-rc.svg)](https://crates.io/crates/hybrid-rc)
[![Documentation](https://docs.rs/hybrid-rc/badge.svg)](https://docs.rs/hybrid-rc/)
[![License](https://img.shields.io/crates/l/hybrid-rc.svg)](https://www.mozilla.org/en-US/MPL/2.0/)
[![Build Status](https://gitlab.com/cg909/rust-hybrid-rc/badges/master/pipeline.svg)](https://gitlab.com/cg909/rust-hybrid-rc/-/commits/master)
[![Test Coverage](https://gitlab.com/cg909/rust-hybrid-rc/badges/master/coverage.svg)](https://cg909.gitlab.io/rust-hybrid-rc/cov/)

## Usage

1. Add the following to your Cargo.toml:
```toml
[dependencies]
hybrid-rc = "0.5.0"
```

2. Read the [crate documentation](https://docs.rs/hybrid-rc/)

## Functionality

The `hybrid-rc` crate provides generic types `HybridRc<T, State>` and `Weak<T>` for reference
counting pointers.

It is loosely based on the algorithm described in
["Biased reference counting: minimizing atomic operations in garbage collection"][doi:10.1145/3243176.3243195]
by Jiho Choi et. al. but adapted to Rust's type system and its lack of a managed runtime
environment.

The switching between atomic and non-atomic reference counting is managed through the type system:
- `HybridRc<T, Local>` (type aliased as `Rc`): very fast but only usable on one thread.
- `HybridRc<T, Shared>` (type aliased as `Arc`): slower but universally usable.

Instances of both variants are convertible into each other. Especially, an `Rc` can always be
converted into an `Arc` using `HybridRc::to_shared(&rc)` or `.into()`.  An `Arc` on the other
hand can only be converted into an `Rc` using `HybridRc::to_local(&arc)` or `.try_into()` if
no other thread has `Rc`s for the same value.

## Tasks

- [ ] All stable `Rc`/`Arc` functionality implemented
  - [x] `new()`
  - [x] `as_ptr()`
  - [x] `get_mut()`
  - [x] `get_mut_unchecked()`
  - [x] `make_mut()`
  - [x] `into_raw()`
  - [x] `from_raw()`
  - [x] `pin()`
  - [x] `try_unwrap()`
  - [x] `weak_count()`
  - [x] `strong_count()`
  - [x] `[in|de]crement_strong_count()` (as `increment_shared_strong_count()`, etc.)
  - [x] `ptr_eq()`
  - [x] `downcast()`
  - [x] `impl AsRef`
  - [x] `impl Borrow`
  - [x] `impl Clone`
  - [x] `impl Debug`
  - [x] `impl Default`
  - [x] `impl Deref`
  - [x] `impl Display`
  - [x] `impl Drop`
  - [x] `impl From<&[T]>`
  - [x] `impl From<&str>`
  - [x] `impl From<String>`
  - [x] `impl From<Box<T>>`
  - [x] `impl From<Cow<'a, T>`
  - [x] `impl From<T>`
  - [x] `impl From<Vec<T>>`
  - [x] `impl FromIterator`
    - [ ] Specialization for `TrustedLen`
  - [ ] `impl Into<Waker>`
  - [x] `impl Hash`
  - [x] `impl Ord`
  - [x] `impl PartialEq`, `impl Eq`
  - [x] `impl PartialOrd`
  - [x] `impl Pointer`
  - [x] `deref()`, `borrow()`, `as_ref()`
  - [x] Weak pointers (`downgrade()`, `upgrade()`)
- [x] Conversion between local and shared `HybridRc`s
  - [x] `to_shared()`, `to_local()`
  - [x] `impl From`, `impl TryFrom`
- [x] Convenience type aliases
- [ ] Unsized coercion
  - [x] Upcasting to `HybridRc<dyn Any, _>` with `From`
  - [x] Converting from `HybridRc<[T; N], _>` to `HybridRc<[T], _>` with `From`
- [ ] Minimize memory overhead
- [x] Cyclic reference creation supported (`new_cyclic()`)
- [x] Full `Pin` support (including upgrading/downgrading to `PinWeak`)
  
# Performance

`Rc::clone()` and `Arc::clone()` as well as `Rc::drop()` and `Arc::drop()` run in virtually
the same time as their standard library counterparts.

`Weak` references are modeled after `std::sync::Weak` and thus always use atomic operations.
`Weak::clone()` and upgrading to an `Arc` perform slightly slower than their standard
library counterparts. `Weak::upgrade_local()` is about 15 % slower than `Weak::upgrade()`.

`Rc::to_shared()` is essentially as expensive as `Arc::clone()`, while `Arc::to_local()` is
about as expensive as `Weak::upgrade()`.

The memory overhead for each allocation is about twice as high than for the standard library
counterparts to accomodate for the additional reference counter and the owner thread id (on
`x86_64` that amounts to 32 bytes per allocation). The pointer objects themselves are as big
as `NonNull<T>`.

## Examplary benchmarks

Benchmarks executed on an Intel Core i7-4790K. YMMV.

|               | `std::rc::Rc`  | `HybridRc<T, Local>` | `HybridRc<T, Shared>` | `std::sync::Arc` |
|---------------|----------------|----------------------|-----------------------|------------------|
| `clone()`     | 1.5 ns         | 1.5 ns               | 5.0 ns                | 5.0 ns           |
| `drop()`      | 1.5 ns         | 1.6 ns               | 4.6 ns                | 6.3 ns           |
| `to_local()`  |                |                      | 8.2 ns                |                  |
| `to_shared()` |                | 5.0 ns               |                       |                  |
| `downgrade()` | 1.5 ns         | 8.0 ns               | 8.0 ns                | 7.8 ns           |
| `upgrade*()`  | 1.6 ns         | 9.4 ns               | 8.2 ns                | 8.0 ns           |

## `no_std` Support

This crate provides limited support for `no_std` environments. In this mode `Arc::to_local()` and
`Weak::upgrade_local()` only succeed if no `Rc` exists on *any* thread, as threads cannot be
reliably identified without `std`.

To enable `no_std` mode, disable the default enabled `std` feature in Cargo.toml. A global
allocator is required.

## Supported Rust versions

The minimum supported Rust toolchain version is Rust **1.55.0**.

The minimum supported Rust toolchain version for `no_std` support is Rust **1.56.0**.

## Stability

This crate follows [semantic versioning](http://semver.org) with the additional
promise that below `1.0.0` backwards-incompatible changes will not be
introduced with only a patch-level version number change.

## License

Licensed under Mozilla Public License, Version 2.0 ([LICENSE](LICENSE)
or https://www.mozilla.org/en-US/MPL/2.0/).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, shall be licensed as above
including compatibility with secondary licenses, as defined by the MPL.

[doi:10.1145/3243176.3243195]: https://dl.acm.org/doi/10.1145/3243176.3243195
