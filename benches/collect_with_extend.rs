use append_only_vec::AppendOnlyVec;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use orx_concurrent_vec::*;

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

fn with_concurrent_vec<T: Sync, P: IntoConcurrentPinnedVec<ConcurrentElement<T>>>(
    num_threads: usize,
    num_items_per_thread: usize,
    compute: fn(usize, usize) -> T,
    batch_size: usize,
    vec: ConcurrentVec<T, P>,
) -> ConcurrentVec<T, P> {
    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| {
                for j in (0..num_items_per_thread).step_by(batch_size) {
                    let into_iter = (j..(j + batch_size)).map(|j| compute(j, j + 1));
                    vec.extend(into_iter);
                }
            });
        }
    });

    vec
}

fn with_rayon<T: Send + Sync + Clone + Copy>(
    num_threads: usize,
    num_items_per_thread: usize,
    compute: fn(usize, usize) -> T,
) -> Vec<T> {
    use rayon::prelude::*;

    let result: Vec<_> = (0..num_threads)
        .into_par_iter()
        .flat_map(|_| {
            (0..num_items_per_thread)
                .map(move |j| compute(j, j + 1))
                .collect::<Vec<_>>()
        })
        .collect();

    result
}

fn with_append_only_vec<T: Send + Sync + Clone + Copy>(
    num_threads: usize,
    num_items_per_thread: usize,
    compute: fn(usize, usize) -> T,
    vec: AppendOnlyVec<T>,
) -> AppendOnlyVec<T> {
    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| {
                for j in 0..num_items_per_thread {
                    vec.push(compute(j, j + 1));
                }
            });
        }
    });

    vec
}

fn with_boxcar<T: Send + Sync + Clone + Copy>(
    num_threads: usize,
    num_items_per_thread: usize,
    compute: fn(usize, usize) -> T,
    vec: boxcar::Vec<T>,
) -> boxcar::Vec<T> {
    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| {
                for j in 0..num_items_per_thread {
                    vec.push(compute(j, j + 1));
                }
            });
        }
    });

    vec
}

fn bench_grow(c: &mut Criterion) {
    let thread_info = [(8, 16384), (8, 65536)];

    let mut group = c.benchmark_group("grow");

    for (num_threads, num_items_per_thread) in thread_info {
        let treatment = format!(
            "num_threads={},num_items_per_thread-type=[{}]",
            num_threads, num_items_per_thread
        );

        let max_len = num_threads * num_items_per_thread;

        let compute = compute_large_data;

        // rayon

        group.bench_with_input(BenchmarkId::new("rayon", &treatment), &(), |b, _| {
            b.iter(|| {
                black_box(with_rayon(
                    black_box(num_threads),
                    black_box(num_items_per_thread),
                    compute,
                ))
            })
        });

        // APPEND-ONLY-VEC

        group.bench_with_input(
            BenchmarkId::new("append_only_vec", &treatment),
            &(),
            |b, _| {
                b.iter(|| {
                    black_box(with_append_only_vec(
                        black_box(num_threads),
                        black_box(num_items_per_thread),
                        compute,
                        AppendOnlyVec::new(),
                    ))
                })
            },
        );

        // BOXCAR

        group.bench_with_input(BenchmarkId::new("boxcar", &treatment), &(), |b, _| {
            b.iter(|| {
                black_box(with_boxcar(
                    black_box(num_threads),
                    black_box(num_items_per_thread),
                    compute,
                    boxcar::Vec::new(),
                ))
            })
        });

        // ConcurrentVec

        let batch_sizes = vec![64, num_items_per_thread];

        for batch_size in batch_sizes {
            let name = |pinned_type: &str| {
                format!(
                    "concurrent_vec({}) | batch-size={}",
                    pinned_type, batch_size
                )
            };

            group.bench_with_input(
                BenchmarkId::new(name("Doubling"), &treatment),
                &(),
                |b, _| {
                    b.iter(|| {
                        black_box(with_concurrent_vec(
                            black_box(num_threads),
                            black_box(num_items_per_thread),
                            compute,
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
                            compute,
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
                        compute,
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
