use orx_concurrent_vec::*;
use orx_fixed_vec::FixedVec;
use orx_pinned_vec::PinnedVec;
use orx_split_vec::SplitVec;
use std::time::Duration;

fn run_with_scope<P: PinnedVec<Option<i32>> + Clone + 'static>(
    pinned: P,
    num_threads: usize,
    num_items_per_thread: usize,
    do_sleep: bool,
) {
    let bag: ConcurrentVec<_, _> = pinned.clone().into();
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
}

#[test]
#[should_panic]
fn out_of_capacity_linear() {
    let (num_threads, num_items_per_thread) = (4, 1024);
    let pinned = SplitVec::with_linear_growth_and_fragments_capacity(5, 10); // up to 320
    run_with_scope(pinned, num_threads, num_items_per_thread, true);
}

#[test]
#[should_panic]
fn out_of_capacity_doubling() {
    let (num_threads, num_items_per_thread) = (5, 15);
    let pinned = SplitVec::with_doubling_growth_and_fragments_capacity(4); // up to 4 + 8 + 16 + 32 = 60
    run_with_scope(pinned, num_threads, num_items_per_thread, true);
}

#[test]
#[should_panic]
fn out_of_capacity_recursive() {
    let (num_threads, num_items_per_thread) = (5, 15);
    let pinned = SplitVec::with_recursive_growth_and_fragments_capacity(4); // up to 4 + 8 + 16 + 32 = 60
    run_with_scope(pinned, num_threads, num_items_per_thread, true);
}

#[test]
#[should_panic]
fn out_of_capacity_fixed() {
    let (num_threads, num_items_per_thread) = (5, 15);
    let pinned = FixedVec::new(74);
    run_with_scope(pinned, num_threads, num_items_per_thread, true);
}

// helpers

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
