use orx_concurrent_vec::*;
use std::{sync::Arc, thread, time::Duration};
use test_case::test_case;

const NUM_RERUNS: usize = 64;

#[test_case(1, 64, false)]
#[test_case(1, 1024, false)]
#[test_case(2, 32, false)]
#[test_case(4, 32, false)]
#[test_case(4, 128, false)]
#[test_case(4, 1024, false)]
#[test_case(8, 4096, false)]
#[test_case(8, 16384, false)]
#[test_case(8, 262144, false)]
#[test_case(1, 64, true)]
#[test_case(1, 1024, true)]
#[test_case(2, 32, true)]
#[test_case(4, 32, true)]
#[test_case(4, 128, true)]
#[test_case(4, 1024, true)]
#[test_case(8, 4096, true)]
#[test_case(8, 16384, true)]
#[test_case(8, 262144, true)]
fn with_arc(num_threads: usize, num_items_per_thread: usize, do_sleep: bool) {
    for _ in 0..NUM_RERUNS {
        let bag = Arc::new(ConcurrentVec::new());
        let mut thread_vec: Vec<thread::JoinHandle<()>> = Vec::new();

        for i in 0..num_threads {
            let bag = bag.clone();
            thread_vec.push(thread::spawn(move || {
                sleep(do_sleep, i);
                for j in 0..num_items_per_thread {
                    bag.push((i * 100000 + j) as i32);
                }
            }));
        }

        for handle in thread_vec {
            handle.join().unwrap();
        }

        assert_result(num_threads, num_items_per_thread, &bag);
    }
}

#[test_case(1, 64, false)]
#[test_case(1, 1024, false)]
#[test_case(2, 32, false)]
#[test_case(4, 32, false)]
#[test_case(4, 128, false)]
#[test_case(4, 1024, false)]
#[test_case(8, 4096, false)]
#[test_case(8, 16384, false)]
#[test_case(8, 262144, false)]
#[test_case(1, 64, true)]
#[test_case(1, 1024, true)]
#[test_case(2, 32, true)]
#[test_case(4, 32, true)]
#[test_case(4, 128, true)]
#[test_case(4, 1024, true)]
#[test_case(8, 4096, true)]
#[test_case(8, 16384, true)]
#[test_case(8, 262144, true)]
fn with_scope(num_threads: usize, num_items_per_thread: usize, do_sleep: bool) {
    for _ in 0..NUM_RERUNS {
        let bag = ConcurrentVec::new();
        let bag_ref = &bag;
        std::thread::scope(|s| {
            for i in 0..num_threads {
                s.spawn(move || {
                    sleep(do_sleep, i);
                    for j in 0..num_items_per_thread {
                        bag_ref.push((i * 100000 + j) as i32);
                    }
                });
            }
        });

        assert_result(num_threads, num_items_per_thread, &bag);
    }
}

// helpers
fn expected_result(num_threads: usize, num_items_per_thread: usize) -> Vec<i32> {
    let mut expected: Vec<_> = (0..num_threads)
        .flat_map(|i| (0..num_items_per_thread).map(move |j| (i * 100000 + j) as i32))
        .collect();
    expected.sort();
    expected
}

fn assert_result(num_threads: usize, num_items_per_thread: usize, bag: &ConcurrentVec<i32>) {
    let mut vec_from_bag: Vec<_> = bag.iter().copied().collect();
    vec_from_bag.sort();

    let expected = expected_result(num_threads, num_items_per_thread);

    assert_eq!(vec_from_bag, expected);
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
