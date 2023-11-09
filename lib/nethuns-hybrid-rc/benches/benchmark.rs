use criterion::{criterion_group, criterion_main, BatchSize::SmallInput, Criterion};
use nethuns_hybrid_rc::{Arc, Rc};
use std::mem::drop;
use std::rc;
use std::sync;

fn bench_cloning(c: &mut Criterion) {
	let mut group = c.benchmark_group("Cloning");
	
	{
		let std_rc = rc::Rc::new(());
		group.bench_function("std::rc::Rc", |b| b.iter_with_large_drop(|| std_rc.clone()));
	}
	
	{
		let our_rc = Rc::new(());
		group.bench_function("hybrid_rc::Rc", |b| {
			b.iter_with_large_drop(|| our_rc.clone())
		});
	}

	{
		let our_arc = Arc::new(());
		group.bench_function("hybrid_rc::Arc", |b| {
			b.iter_with_large_drop(|| our_arc.clone())
		});
	}

	{
		let arc = sync::Arc::new(());
		group.bench_function("std::sync::Arc", |b| b.iter_with_large_drop(|| arc.clone()));
	}

	group.finish();
}

fn bench_dropping(c: &mut Criterion) {
	let mut group = c.benchmark_group("Dropping");

	{
		let std_rc = rc::Rc::new(());
		group.bench_function("std::rc::Rc", |b| {
			b.iter_batched(|| std_rc.clone(), |r| drop(r), SmallInput)
		});
	}

	{
		let our_rc = Rc::new(());
		group.bench_function("hybrid_rc::Rc", |b| {
			b.iter_batched(|| our_rc.clone(), |r| drop(r), SmallInput)
		});
	}

	{
		let our_arc = Arc::new(());
		group.bench_function("hybrid_rc::Arc", |b| {
			b.iter_batched(|| our_arc.clone(), |r| drop(r), SmallInput)
		});
	}

	{
		let arc = sync::Arc::new(());
		group.bench_function("std::sync::Arc", |b| {
			b.iter_batched(|| arc.clone(), |r| drop(r), SmallInput)
		});
	}

	group.finish();
}

fn bench_weak_cloning(c: &mut Criterion) {
	let mut group = c.benchmark_group("Weak cloning");

	{
		let rc = rc::Rc::new(());
		let weak = rc::Rc::downgrade(&rc);
		group.bench_function("std::rc::Weak", |b| b.iter_with_large_drop(|| weak.clone()));
	}

	{
		let rc = Rc::new(());
		let weak = Rc::downgrade(&rc);
		group.bench_function("hybrid_rc::Weak", |b| {
			b.iter_with_large_drop(|| weak.clone())
		});
	}

	{
		let rc = sync::Arc::new(());
		let weak = sync::Arc::downgrade(&rc);
		group.bench_function("std::sync::Weak", |b| {
			b.iter_with_large_drop(|| weak.clone())
		});
	}

	group.finish();
}

fn bench_upgrading(c: &mut Criterion) {
	let mut group = c.benchmark_group("Upgrading");

	{
		let rc = rc::Rc::new(());
		let weak = rc::Rc::downgrade(&rc);
		group.bench_function("std::rc::Weak", |b| {
			b.iter_with_large_drop(|| weak.upgrade())
		});
	}

	{
		let rc = Rc::new(());
		let weak = Rc::downgrade(&rc);
		group.bench_function("hybrid_rc::Weak (local)", |b| {
			b.iter_with_large_drop(|| weak.upgrade_local())
		});
	}

	{
		let rc = Arc::new(());
		let weak = Arc::downgrade(&rc);
		group.bench_function("hybrid_rc::Weak (local, ownerless)", |b| {
			b.iter_with_large_drop(|| weak.upgrade_local())
		});
	}

	{
		let rc = Rc::new(());
		let weak = Rc::downgrade(&rc);
		group.bench_function("hybrid_rc::Weak (shared)", |b| {
			b.iter_with_large_drop(|| weak.upgrade())
		});
	}

	{
		let rc = sync::Arc::new(());
		let weak = sync::Arc::downgrade(&rc);
		group.bench_function("std::sync::Weak", |b| {
			b.iter_with_large_drop(|| weak.upgrade())
		});
	}

	group.finish();
}

fn bench_downgrading(c: &mut Criterion) {
	let mut group = c.benchmark_group("Downgrading");

	{
		let rc = rc::Rc::new(());
		group.bench_function("std::rc::Rc", |b| {
			b.iter_with_large_drop(|| rc::Rc::downgrade(&rc))
		});
	}

	{
		let rc = Rc::new(());
		group.bench_function("hybrid_rc::Rc", |b| {
			b.iter_with_large_drop(|| Rc::downgrade(&rc))
		});
	}

	{
		let rc = Arc::new(());
		group.bench_function("hybrid_rc::Arc", |b| {
			b.iter_with_large_drop(|| Arc::downgrade(&rc))
		});
	}

	{
		let rc = sync::Arc::new(());
		group.bench_function("std::sync::Arc", |b| {
			b.iter_with_large_drop(|| sync::Arc::downgrade(&rc))
		});
	}

	group.finish();
}

fn bench_switching(c: &mut Criterion) {
	let mut group = c.benchmark_group("Switching");

	{
		let rc = Rc::new(());
		group.bench_function("Local to shared", |b| {
			b.iter_with_large_drop(|| Rc::to_shared(&rc))
		});
	}

	{
		let rc_owner = Rc::new(());
		let arc = Rc::to_shared(&rc_owner);
		group.bench_function("Shared to local", |b| {
			b.iter_with_large_drop(|| Arc::to_local(&arc))
		});
	}

	{
		let rc = Arc::new(());
		group.bench_function("Shared to local (ownerless)", |b| {
			b.iter_with_large_drop(|| Arc::to_local(&rc))
		});
	}

	group.finish();
}

criterion_group!(
	benches,
	bench_cloning,
	bench_dropping,
	bench_weak_cloning,
	bench_switching,
	bench_upgrading,
	bench_downgrading
);
criterion_main!(benches);
