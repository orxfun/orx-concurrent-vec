use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orx_concurrent_vec::*;
use std::time::Duration;

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct LargeData {
    a: [i32; 64],
}

#[allow(dead_code)]
fn compute_data_i32(i: usize, j: usize) -> i32 {
    (i * 100000 + j) as i32
}

#[allow(dead_code)]
fn compute_large_data(i: usize, j: usize) -> LargeData {
    let mut a = [0i32; 64];

    #[allow(clippy::needless_range_loop)]
    for k in 0..64 {
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

fn with_concurrent_vec<T: Sync, P: PinnedVec<Option<T>>>(
    num_threads: usize,
    num_items_per_thread: usize,
    do_sleep: bool,
    compute: fn(usize, usize) -> T,
    batch_size: usize,
    bag: ConcurrentVec<T, P>,
) -> ConcurrentVec<T, P> {
    let bag_ref = &bag;
    std::thread::scope(|s| {
        for i in 0..num_threads {
            s.spawn(move || {
                sleep(do_sleep, i);
                for j in (0..num_items_per_thread).step_by(batch_size) {
                    let into_iter =
                        (j..(j + batch_size)).map(|j| std::hint::black_box(compute(i, j)));
                    bag_ref.extend(into_iter);
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
                .map(move |j| std::hint::black_box(compute(i, j)))
                .collect::<Vec<_>>()
        })
        .collect();

    result
}

fn sleep(do_sleep: bool, i: usize) {
    if do_sleep {
        let modulus = i % 3;
        let nanoseconds = match modulus {
            0 => 0,
            1 => 10 + (i % 11) * 4,
            _ => 20 - (i % 5) * 3,
        } as u64;
        let duration = Duration::from_nanos(nanoseconds);
        std::thread::sleep(duration);
    }
}

fn bench_grow(c: &mut Criterion) {
    let thread_info: [(usize, usize); 2] = [(8, 16384), (8, 65536)];
    let workload_info = [false, true];

    let treatments: Vec<_> = workload_info
        .iter()
        .flat_map(|&w| thread_info.iter().map(move |(a, b)| (*a, *b, w)))
        .collect();

    let mut group = c.benchmark_group("collect-extend");

    for (num_threads, num_items_per_thread, add_workload) in treatments {
        let treatment = format!(
            "workload={},num_threads={},num_items_per_thread-type=[{}]",
            add_workload, num_threads, num_items_per_thread
        );

        let max_len = num_threads * num_items_per_thread;

        // rayon

        group.bench_with_input(BenchmarkId::new("with_rayon", &treatment), &(), |b, _| {
            b.iter(|| {
                black_box(with_rayon(
                    black_box(num_threads),
                    black_box(num_items_per_thread),
                    add_workload,
                    compute_large_data,
                ))
            })
        });

        // ConcurrentBag

        let batch_sizes = vec![1, 2, 4, 16, 64, num_items_per_thread];

        for batch_size in batch_sizes {
            let name =
                |pinned_type: &str| format!("convec({}) | batch={}", pinned_type, batch_size);

            group.bench_with_input(
                BenchmarkId::new(name("Doubling"), &treatment),
                &(),
                |b, _| {
                    b.iter(|| {
                        black_box(with_concurrent_vec(
                            black_box(num_threads),
                            black_box(num_items_per_thread),
                            add_workload,
                            compute_large_data,
                            batch_size,
                            ConcurrentVec::with_doubling_growth(),
                        ))
                    })
                },
            );

            let fragment_size = 2usize.pow(12);
            let num_linear_fragments = (max_len / fragment_size) + 1;
            group.bench_with_input(
                BenchmarkId::new(name("Linear(12)"), &treatment),
                &(),
                |b, _| {
                    b.iter(|| {
                        black_box(with_concurrent_vec(
                            black_box(num_threads),
                            black_box(num_items_per_thread),
                            add_workload,
                            compute_large_data,
                            batch_size,
                            ConcurrentVec::with_linear_growth(12, num_linear_fragments),
                        ))
                    })
                },
            );

            group.bench_with_input(BenchmarkId::new(name("Fixed"), &treatment), &(), |b, _| {
                b.iter(|| {
                    black_box(with_concurrent_vec(
                        black_box(num_threads),
                        black_box(num_items_per_thread),
                        add_workload,
                        compute_large_data,
                        batch_size,
                        ConcurrentVec::with_fixed_capacity(num_threads * num_items_per_thread),
                    ))
                })
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_grow);
criterion_main!(benches);
