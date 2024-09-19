use orx_concurrent_option::ConcurrentOption;
use orx_concurrent_vec::*;
use std::time::Duration;
use test_case::test_case;

fn concurrent_read_and_extend<
    P: IntoConcurrentPinnedVec<ConcurrentOption<String>> + Clone + 'static,
>(
    pinned: P,
    num_threads: usize,
    num_items_per_thread: usize,
    do_sleep: bool,
) {
    for _ in 0..NUM_RERUNS {
        let vec: ConcurrentVec<_, _> = pinned.clone().into();
        let vec_ref = &vec;
        std::thread::scope(|s| {
            // reader thread: continuously reads while other threads write
            s.spawn(move || {
                let final_len = num_threads * num_items_per_thread;
                while vec_ref.len() < final_len {
                    #[allow(unused_variables)]
                    let mut sum = 0i64;

                    // it is safe to query indices outside the bounds
                    let len = vec_ref.len() + 100;
                    for i in 0..len {
                        // random access via get or [i]
                        if let Some(value) = vec_ref.get(i) {
                            sum += value.len() as i64;
                        }
                    }

                    // or sequential access via iter
                    for value in vec_ref.iter() {
                        sum -= value.len() as i64;
                    }
                }
            });

            // writer threads: keeps growing the vec
            let batch_size = 32;
            for i in 0..num_threads {
                s.spawn(move || {
                    sleep(do_sleep, i);
                    // instead of pushing 1-by-1; extend by an iterator yielding batches of 32 elements
                    // see here for a relevant discussion on false sharing:
                    // https://docs.rs/orx-concurrent-bag/latest/orx_concurrent_bag/#false-sharing
                    for j in (0..num_items_per_thread).step_by(batch_size) {
                        let into_iter = (j..(j + batch_size)).map(|j| (i * 100000 + j).to_string());
                        vec_ref.extend(into_iter);
                    }
                });
            }
        });

        assert_result(num_threads, num_items_per_thread, vec.into_inner());
    }
}

const NUM_RERUNS: usize = 1;

#[cfg(not(miri))]
const EXHAUSTIVE_INPUTS: [(usize, usize); 8] = [
    (1, 64),
    (1, 1024),
    (2, 32),
    (4, 32),
    (4, 128),
    (4, 1024),
    (8, 4096),
    (16, 1024),
];

const FAST_INPUTS: [(usize, usize); 6] = [(1, 64), (1, 256), (2, 32), (4, 32), (4, 128), (8, 256)];

fn run_test<P: IntoConcurrentPinnedVec<ConcurrentOption<String>> + Clone + 'static>(
    pinned: P,
    inputs: &[(usize, usize)],
) {
    for sleep in [false, true] {
        for (num_threads, num_items_per_thread) in inputs {
            concurrent_read_and_extend(pinned.clone(), *num_threads, *num_items_per_thread, sleep);
        }
    }
}

#[test_case(FixedVec::new(65536))]
#[test_case(SplitVec::with_doubling_growth_and_fragments_capacity(32))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(6, 8192))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(10, 512))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(14, 32))]
#[cfg(not(miri))]
fn concurrent_extend_read_exhaustive<
    P: IntoConcurrentPinnedVec<ConcurrentOption<String>> + Clone + 'static,
>(
    pinned: P,
) {
    run_test(pinned, &EXHAUSTIVE_INPUTS)
}

#[test_case(FixedVec::new(3000))]
#[test_case(SplitVec::with_doubling_growth_and_fragments_capacity(32))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(6, 40))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(10, 10))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(14, 10))]
fn concurrent_extend_read_fast<
    P: IntoConcurrentPinnedVec<ConcurrentOption<String>> + Clone + 'static,
>(
    pinned: P,
) {
    run_test(pinned, &FAST_INPUTS)
}

// helpers
fn expected_result(num_threads: usize, num_items_per_thread: usize) -> Vec<String> {
    let mut expected: Vec<_> = (0..num_threads)
        .flat_map(|i| (0..num_items_per_thread).map(move |j| (i * 100000 + j).to_string()))
        .collect();
    expected.sort();
    expected
}

fn assert_result<P: PinnedVec<ConcurrentOption<String>>>(
    num_threads: usize,
    num_items_per_thread: usize,
    mut vec_from_bag: P,
) {
    vec_from_bag.sort();
    let vec_from_bag: Vec<_> = vec_from_bag.into_iter().map(|x| x.unwrap()).collect();

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
