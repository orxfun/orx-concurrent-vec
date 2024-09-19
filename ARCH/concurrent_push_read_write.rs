use orx_concurrent_option::ConcurrentOption;
use orx_concurrent_vec::*;
use std::time::Duration;
use test_case::test_case;

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

const NUM_RERUNS: usize = 1;

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

fn concurrent_push_and_read_and_write<
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

                    // it is SAFE to query indices outside the bounds
                    let len = vec_ref.len() + 0;
                    for i in 0..len {
                        if let Some(value) = vec_ref.get_cloned(i) {
                            sum += value.len() as i64;
                        }
                    }

                    // for value in vec_ref.iter() {
                    //     sum -= value.len() as i64;
                    // }
                }
            });

            // writer thread:: continuously updates already added elements
            s.spawn(move || {
                let final_len = num_threads * num_items_per_thread;
                let mut alternating = false;
                while vec_ref.len() < final_len {
                    alternating = !alternating;

                    // it is SAFE to query indices outside the bounds
                    let len = vec_ref.len() + 100;
                    for i in 0..len {
                        if let Some(value) = vec_ref.get(i) {
                            let value: i64 = value.parse().unwrap();
                            let new_value = match alternating {
                                true => value + 1,
                                false => value - 1,
                            }
                            .to_string();
                            let _ = vec_ref.replace(i, new_value);
                        }
                    }
                }
            });

            // writer threads: keeps growing the vec
            for i in 0..num_threads {
                s.spawn(move || {
                    sleep(do_sleep, i);
                    for j in 0..num_items_per_thread {
                        vec_ref.push((i * 100000 + j).to_string());
                    }
                });
            }
        });
    }
}

fn run_test<P: IntoConcurrentPinnedVec<ConcurrentOption<String>> + Clone + 'static>(
    pinned: P,
    inputs: &[(usize, usize)],
) {
    for sleep in [false, true] {
        for (num_threads, num_items_per_thread) in inputs {
            concurrent_push_and_read_and_write(
                pinned.clone(),
                *num_threads,
                *num_items_per_thread,
                sleep,
            );
        }
    }
}

// #[test_case(FixedVec::new(3000))]
#[test_case(SplitVec::with_doubling_growth_and_fragments_capacity(32))]
// #[test_case(SplitVec::with_linear_growth_and_fragments_capacity(6, 40))]
// #[test_case(SplitVec::with_linear_growth_and_fragments_capacity(10, 10))]
// #[test_case(SplitVec::with_linear_growth_and_fragments_capacity(14, 10))]
fn concurrent_push_and_read_and_write_fast<
    P: IntoConcurrentPinnedVec<ConcurrentOption<String>> + Clone + 'static,
>(
    pinned: P,
) {
    run_test(pinned, &FAST_INPUTS)
}
