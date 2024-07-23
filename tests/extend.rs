use orx_concurrent_vec::*;
use orx_fixed_vec::FixedVec;
use orx_pinned_vec::IntoConcurrentPinnedVec;
use orx_split_vec::SplitVec;
use std::time::Duration;
use test_case::test_case;

const NUM_RERUNS: usize = 1;

const EXHAUSTIVE_INPUTS: [(usize, usize); 15] = [
    (1, 64),
    (1, 1024),
    (1, 65536),
    (2, 32),
    (4, 32),
    (4, 128),
    (4, 1024),
    (8, 4096),
    (8, 16384),
    (8, 65536),
    (16, 1024),
    (16, 4096),
    (16, 16384),
    (32, 1024),
    (64, 1024),
];

const FAST_INPUTS: [(usize, usize); 6] = [(1, 64), (1, 256), (2, 32), (4, 32), (4, 128), (8, 256)];

fn run_with_scope<P: IntoConcurrentPinnedVec<i32> + Clone + 'static>(
    pinned: P,
    num_threads: usize,
    num_items_per_thread: usize,
    do_sleep: bool,
) {
    for _ in 0..NUM_RERUNS {
        let bag: ConcurrentVec<_, _> = pinned.clone().into();
        let bag_ref = &bag;
        std::thread::scope(|s| {
            for i in 0..num_threads {
                s.spawn(move || {
                    sleep(do_sleep, i);
                    let into_iter = (0..num_items_per_thread).map(|j| (i * 100000 + j) as i32);
                    bag_ref.extend(into_iter);
                });
            }
        });

        assert_result(num_threads, num_items_per_thread, bag.into_inner());
    }
}

fn run_test<P: IntoConcurrentPinnedVec<i32> + Clone + 'static>(
    pinned: P,
    inputs: &[(usize, usize)],
) {
    for sleep in [false, true] {
        for (num_threads, num_items_per_thread) in inputs.iter() {
            run_with_scope(pinned.clone(), *num_threads, *num_items_per_thread, sleep);
        }
    }
}

// #[test_case(FixedVec::new(524288))]
// #[test_case(SplitVec::with_doubling_growth_and_fragments_capacity(32))]
// #[test_case(SplitVec::with_linear_growth_and_fragments_capacity(6, 8192))]
// #[test_case(SplitVec::with_linear_growth_and_fragments_capacity(10, 512))]
// #[test_case(SplitVec::with_linear_growth_and_fragments_capacity(14, 32))]
#[allow(dead_code)]
fn push_exhaustive<P: IntoConcurrentPinnedVec<i32> + Clone + 'static>(pinned: P) {
    run_test(pinned, &EXHAUSTIVE_INPUTS)
}

#[test_case(FixedVec::new(3000))]
#[test_case(SplitVec::with_doubling_growth_and_fragments_capacity(32))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(6, 40))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(10, 10))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(14, 10))]
fn push_fast<P: IntoConcurrentPinnedVec<i32> + Clone + 'static>(pinned: P) {
    run_test(pinned, &FAST_INPUTS)
}

// helpers
fn expected_result(num_threads: usize, num_items_per_thread: usize) -> Vec<i32> {
    let mut expected: Vec<_> = (0..num_threads)
        .flat_map(|i| (0..num_items_per_thread).map(move |j| (i * 100000 + j) as i32))
        .collect();
    expected.sort();
    expected
}

fn assert_result<P: IntoConcurrentPinnedVec<i32>>(
    num_threads: usize,
    num_items_per_thread: usize,
    vec_from_bag: P,
) {
    let mut vec_from_bag: Vec<_> = vec_from_bag.iter().copied().collect();
    vec_from_bag.sort();

    let expected = expected_result(num_threads, num_items_per_thread);

    assert_eq!(vec_from_bag.len(), expected.len());

    assert!(vec_from_bag == expected);
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
