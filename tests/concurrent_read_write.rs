mod utils;
use orx_concurrent_vec::*;
use orx_split_vec::SplitVec;
use test_case::test_matrix;
use utils::{grower, select, sleep, Elem};

const NUM_GROWERS: usize = 4;
const NUM_USERS: usize = 4;
const BATCH_SIZE: usize = 64;

// push | extend
const FREQ_GROW: [usize; 2] = [2, 1];

// read | write | iterate
const FREQ_USE: [usize; 3] = [1, 0, 0];

fn reader<P>(vec: &ConcurrentVec<String, P>, expected_len: usize)
where
    P: IntoConcurrentPinnedVec<Elem> + 'static,
{
    while vec.len() < expected_len {
        for i in 0..vec.len() {
            match i % 2 == 0 {
                true => {
                    let cloned = vec.get_cloned(i);
                    // assert_eq!(cloned, Some(i.to_string()));
                }
                false => {
                    let correct = vec[i].map(|e| e == &i.to_string());
                    // assert!(correct);
                }
            }
        }
        sleep(true, 42);
    }
}

fn writer<P>(vec: &ConcurrentVec<String, P>, expected_len: usize)
where
    P: IntoConcurrentPinnedVec<Elem> + 'static,
{
    while vec.len() < expected_len {
        for i in 0..vec.len() {
            let new_value = i.to_string();
            match i % 2 == 0 {
                true => vec[i].set(new_value),
                false => {
                    let old = vec[i].replace(new_value);
                    assert_eq!(old, i.to_string());
                }
            }
        }
        sleep(true, 42);
    }
}

fn iterator<P>(vec: &ConcurrentVec<String, P>, expected_len: usize)
where
    P: IntoConcurrentPinnedVec<Elem> + 'static,
{
    while vec.len() < expected_len {
        let iter = vec.iter();

        for (i, elem) in iter.enumerate() {
            match i % 2 == 0 {
                true => elem.set(i.to_string()),
                false => {
                    let cloned = elem.clone();
                    assert_eq!(cloned, i.to_string());
                }
            }
        }
        sleep(true, 42);
    }
}

fn spawn<P>(pinned: P, num_growers: usize, num_users: usize, num_items_per_thread: usize)
where
    P: IntoConcurrentPinnedVec<Elem> + 'static,
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
}

// #[cfg(not(miri))]
#[test_matrix([1_000])]
fn con_grow_use(num_items_per_thread: usize) {
    for _ in 0..100 {
        let pinned = SplitVec::with_doubling_growth_and_fragments_capacity(32);
        spawn(pinned, NUM_GROWERS, NUM_USERS, num_items_per_thread);
    }
}

// #[test_matrix(
//     [
//         FixedVec::new(2_000),
//         SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [250]
// )]
// fn miri_con_grow_use<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_USERS, num_items_per_thread);
// }
