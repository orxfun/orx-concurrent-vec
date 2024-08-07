use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
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

fn with_concurrent_vec<T: Sync, P: IntoConcurrentPinnedVec<T>>(
    num_threads: usize,
    num_items_per_thread: usize,
    compute: fn(usize, usize) -> T,
    bag: ConcurrentVec<T, P>,
) -> ConcurrentVec<T, P> {
    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| {
                for j in 0..num_items_per_thread {
                    bag.push(std::hint::black_box(compute(j, j + 1)));
                }
            });
        }
    });

    bag
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
                .map(move |j| std::hint::black_box(compute(j, j + 1)))
                .collect::<Vec<_>>()
        })
        .collect();

    result
}

fn bench_grow(c: &mut Criterion) {
    let thread_info = [(4, 16384), (4, 65536)];

    let mut group = c.benchmark_group("grow");

    for (num_threads, num_items_per_thread) in thread_info {
        let treatment = format!(
            "num_threads={},num_items_per_thread-type=[{}]",
            num_threads, num_items_per_thread
        );

        let max_len = num_threads * num_items_per_thread;

        // RAYON

        group.bench_with_input(BenchmarkId::new("with_rayon", &treatment), &(), |b, _| {
            b.iter(|| {
                black_box(with_rayon(
                    black_box(num_threads),
                    black_box(num_items_per_thread),
                    compute_large_data,
                ))
            })
        });

        // WITH-SCOPE

        group.bench_with_input(
            BenchmarkId::new("with_concurrent_vec(Doubling)", &treatment),
            &(),
            |b, _| {
                b.iter(|| {
                    black_box(with_concurrent_vec(
                        black_box(num_threads),
                        black_box(num_items_per_thread),
                        compute_large_data,
                        ConcurrentVec::with_doubling_growth(),
                    ))
                })
            },
        );

        let fragment_size = 2usize.pow(12);
        let num_linear_fragments = (max_len / fragment_size) + 1;
        group.bench_with_input(
            BenchmarkId::new("with_concurrent_vec(Linear(12))", &treatment),
            &(),
            |b, _| {
                b.iter(|| {
                    black_box(with_concurrent_vec(
                        black_box(num_threads),
                        black_box(num_items_per_thread),
                        compute_large_data,
                        ConcurrentVec::with_linear_growth(12, num_linear_fragments),
                    ))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("with_concurrent_vec(Fixed)", &treatment),
            &(),
            |b, _| {
                b.iter(|| {
                    black_box(with_concurrent_vec(
                        black_box(num_threads),
                        black_box(num_items_per_thread),
                        compute_large_data,
                        ConcurrentVec::with_fixed_capacity(num_threads * num_items_per_thread),
                    ))
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_grow);
criterion_main!(benches);
