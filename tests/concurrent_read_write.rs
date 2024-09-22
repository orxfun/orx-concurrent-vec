use orx_concurrent_vec::*;
use orx_split_vec::SplitVec;
use std::time::Duration;
use test_case::test_matrix;
// use utils::{grower, select, sleep, ConcurrentElement<String>};

const NUM_GROWERS: usize = 4;
const NUM_USERS: usize = 8;
const BATCH_SIZE: usize = 64;

// push | extend
const FREQ_GROW: [usize; 2] = [1, 1];

// read | write | iterate
const FREQ_USE: [usize; 3] = [1, 1, 1];

// get | get_cloned | map
const FREQ_READ: [usize; 3] = [1, 1, 1];

// replace | set | update
const FREQ_WRITE: [usize; 3] = [1, 1, 1];

fn select<const N: usize>(distribution: &[usize; N], index: usize) -> usize {
    let sum: usize = distribution.iter().sum();
    let m = index % sum;
    let mut partial_sum = 0;
    for (i, x) in distribution.iter().enumerate() {
        partial_sum += x;
        if m < partial_sum {
            return i;
        }
    }
    distribution.len() - 1
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

fn grower<P>(vec: &ConcurrentVec<String, P>, num_items_to_add: usize, batch_size: Option<usize>)
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<String>> + 'static,
{
    match batch_size {
        None => {
            for j in 0..num_items_to_add {
                vec.push_for_idx(|i| i.to_string());
                if j % 95 == 0 {
                    sleep(true, j);
                }
            }
        }
        Some(batch_size) => {
            let mut e = 0;
            let mut j = 0;
            while j < num_items_to_add {
                let extend_len = match num_items_to_add - j > batch_size {
                    true => batch_size,
                    false => num_items_to_add - j,
                };
                let iter =
                    |begin_idx: usize| (begin_idx..(begin_idx + extend_len)).map(|j| j.to_string());
                let _ = vec.extend_for_idx(iter, extend_len);

                j += extend_len;

                e += 1;
                sleep(e % 2 == 0, j);
            }
        }
    }
}

fn reader<P>(vec: &ConcurrentVec<String, P>, expected_len: usize)
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<String>> + 'static,
{
    let read_loop = || {
        let slice = vec.as_slice();
        for i in 0..slice.len() {
            match select(&FREQ_READ, i) {
                0 => {
                    // get
                    let elem = slice.get(i);
                    let cloned = elem.map(|e| e.cloned());
                    assert_eq!(cloned, Some(i.to_string()));
                }
                1 => {
                    // get_cloned
                    let cloned = slice.get_cloned(i);
                    assert_eq!(cloned, Some(i.to_string()));
                }
                2 => {
                    // map
                    let is_equal = slice[i] == i.to_string();
                    assert_eq!(is_equal, true);
                }
                _ => {}
            }
        }
        sleep(true, 42);
    };

    while vec.len() < expected_len {
        read_loop();
    }
}

fn writer<P>(vec: &ConcurrentVec<String, P>, expected_len: usize)
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<String>> + 'static,
{
    let write_loop = || {
        let slice = vec.as_slice();
        for i in 0..slice.len() {
            match select(&FREQ_WRITE, i) {
                0 => {
                    // replace
                    let new_value = i.to_string();
                    let old_value = slice[i].replace(new_value);
                    assert_eq!(old_value, i.to_string());
                }
                1 => {
                    // set
                    slice[i].set(i.to_string());
                }
                2 => {
                    // update
                    slice[i].update(|x| {
                        let number: usize = x.parse().unwrap();
                        *x = number.to_string();
                    });
                }
                _ => {}
            }
        }
        sleep(true, 42);
    };

    while vec.len() < expected_len {
        write_loop();
    }
}

fn iterator<P>(vec: &ConcurrentVec<String, P>, expected_len: usize)
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<String>> + 'static,
{
    let iter_loop = || {
        let slice = vec.as_slice();
        for (i, e) in slice.iter().enumerate() {
            match i % 2 == 0 {
                // read
                true => assert!(e == &i.to_string()),
                // or write
                false => {
                    e.update(|x| {
                        let number: usize = x.parse().unwrap();
                        *x = number.to_string();
                    });
                }
            }
        }
    };

    while vec.len() < expected_len {
        iter_loop();
    }
}

fn spawn<P>(pinned: P, num_growers: usize, num_users: usize, num_items_per_thread: usize)
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<String>> + 'static,
{
    let vec: ConcurrentVec<_, _> = pinned.into();
    let con_vec = &vec;
    let expected_len = num_growers * num_items_per_thread;

    std::thread::scope(|s| {
        for i in 0..num_growers {
            let batch_size = match select(&FREQ_GROW, i) {
                0 => None,
                _ => Some(BATCH_SIZE),
            };
            s.spawn(move || grower(con_vec, num_items_per_thread, batch_size));
        }

        for i in 0..num_users {
            match select(&FREQ_USE, i) {
                0 => s.spawn(move || reader(con_vec, expected_len)),
                1 => s.spawn(move || writer(con_vec, expected_len)),
                _ => s.spawn(move || iterator(con_vec, expected_len)),
            };
        }
    });

    assert_eq!(vec.len(), expected_len);
    for (i, x) in vec.iter().enumerate() {
        assert_eq!(x.cloned(), i.to_string());
    }

    let vec = vec.into_inner();
    assert_eq!(vec.len(), expected_len);
    for (i, x) in vec.iter().enumerate() {
        assert_eq!(x.cloned(), i.to_string());
    }
}

#[cfg(not(miri))]
#[test_matrix([500, 1_000, 10_000])]
fn concurrent_read_write(num_items_per_thread: usize) {
    let pinned = SplitVec::with_doubling_growth_and_fragments_capacity(32);
    spawn(pinned, NUM_GROWERS, NUM_USERS, num_items_per_thread);
}

#[test_matrix([250])]
fn miri_concurrent_read_write(num_items_per_thread: usize) {
    let pinned = SplitVec::with_doubling_growth_and_fragments_capacity(32);
    spawn(pinned, NUM_GROWERS, NUM_USERS, num_items_per_thread);
}
