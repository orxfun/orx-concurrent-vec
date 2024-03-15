use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orx_concurrent_vec::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct LargeData {
    a: [i32; 32],
}

#[allow(dead_code)]
fn compute_data_i32(i: usize, j: usize) -> i32 {
    (i * 100000 + j) as i32
}

#[allow(dead_code)]
fn compute_large_data(i: usize, j: usize) -> LargeData {
    let mut a = [0i32; 32];

    #[allow(clippy::needless_range_loop)]
    for k in 0..32 {
        if k == i {
            a[k] = (i - j) as i32;
        } else if k == j {
            a[k] = (j + i) as i32;
        } else {
            a[k] = (i + j + k) as i32;
        }
    }

    LargeData { a }
}

fn with_arc<T: 'static>(
    num_threads: usize,
    num_items_per_thread: usize,
    do_sleep: bool,
    compute: fn(usize, usize) -> T,
) -> Arc<ConcurrentVec<T>> {
    let bag = Arc::new(ConcurrentVec::new());
    let mut thread_vec: Vec<thread::JoinHandle<()>> = Vec::new();

    for i in 0..num_threads {
        let bag = bag.clone();
        thread_vec.push(thread::spawn(move || {
            sleep(do_sleep, i);
            for j in 0..num_items_per_thread {
                bag.push(compute(i, j));
            }
        }));
    }

    for handle in thread_vec {
        handle.join().unwrap();
    }

    bag
}

fn with_scope<T>(
    num_threads: usize,
    num_items_per_thread: usize,
    do_sleep: bool,
    compute: fn(usize, usize) -> T,
) -> ConcurrentVec<T> {
    let bag = ConcurrentVec::new();

    let bag_ref = &bag;
    std::thread::scope(|s| {
        for i in 0..num_threads {
            s.spawn(move || {
                sleep(do_sleep, i);
                for j in 0..num_items_per_thread {
                    bag_ref.push(compute(i, j));
                }
            });
        }
    });

    bag
}

fn with_rayon<T: Send + Sync + Clone + Copy>(
    num_threads: usize,
    num_items_per_thread: usize,
    do_sleep: bool,
    compute: fn(usize, usize) -> T,
) -> Vec<T> {
    use rayon::prelude::*;

    let result: Vec<_> = (0..num_threads)
        .into_par_iter()
        .flat_map(|i| {
            sleep(do_sleep, i);
            (0..num_items_per_thread)
                .map(move |j| compute(i, j))
                .collect::<Vec<_>>()
        })
        .collect();

    result
}

fn sleep(do_sleep: bool, i: usize) {
    if do_sleep {
        let modulus = i % 3;
        let milliseconds = match modulus {
            0 => 0,
            1 => 10 + (i % 11) * 4,
            _ => 20 - (i % 5) * 3,
        } as u64;
        let duration = Duration::from_millis(milliseconds);
        std::thread::sleep(duration);
    }
}

fn bench_grow(c: &mut Criterion) {
    let treatments = vec![(64, 4096), (64, 16384)];

    let add_workload = true;
    let mut group = c.benchmark_group("grow");

    for (num_threads, num_items_per_thread) in treatments {
        let treatment = format!(
            "num_threads={},num_items_per_thread-type=[{}]",
            num_threads, num_items_per_thread
        );

        group.bench_with_input(BenchmarkId::new("with_arc", &treatment), &(), |b, _| {
            b.iter(|| {
                black_box(with_arc(
                    black_box(num_threads),
                    black_box(num_items_per_thread),
                    add_workload,
                    compute_data_i32,
                ))
            })
        });

        group.bench_with_input(BenchmarkId::new("with_scope", &treatment), &(), |b, _| {
            b.iter(|| {
                black_box(with_scope(
                    black_box(num_threads),
                    black_box(num_items_per_thread),
                    add_workload,
                    compute_data_i32,
                ))
            })
        });

        group.bench_with_input(BenchmarkId::new("with_rayon", &treatment), &(), |b, _| {
            b.iter(|| {
                black_box(with_rayon(
                    black_box(num_threads),
                    black_box(num_items_per_thread),
                    add_workload,
                    compute_data_i32,
                ))
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_grow);
criterion_main!(benches);
