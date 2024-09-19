// mod utils;
// use orx_concurrent_vec::*;
// use test_case::test_matrix;
// use utils::{destruct_elem, grow, sleep, Elem};

// const NUM_GROWERS: usize = 4;
// const NUM_USERS: usize = 4;
// const BATCH_SIZE: usize = 64;

// fn use_single_element(value: &String, num_pushers: usize, num_items_per_thread: usize) {
//     let (thread_idx, j) = destruct_elem(value);
//     assert!(thread_idx < num_pushers);
//     assert!(j < num_items_per_thread);
// }

// fn use_elements<P>(vec: &ConcurrentVec<String, P>, num_pushers: usize, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let expected_len = num_pushers * num_items_per_thread;
//     while vec.len() < expected_len {
//         vec.use_elements(|x| use_single_element(x, num_pushers, num_items_per_thread));
//         sleep(true, 42);
//     }
// }

// fn spawn<P>(pinned: P, num_pushers: usize, num_users: usize, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let vec: ConcurrentVec<_, _> = pinned.clone().into();
//     let con_vec = &vec;
//     let expected_len = num_pushers * num_items_per_thread;

//     std::thread::scope(|s| {
//         for i in 0..num_pushers {
//             s.spawn(move || grow(con_vec, i, num_items_per_thread, BATCH_SIZE));
//         }

//         for _ in 0..num_users {
//             s.spawn(move || use_elements(con_vec, num_pushers, num_items_per_thread));
//         }
//     });

//     assert_eq!(vec.len(), expected_len);
// }

// #[cfg(not(miri))]
// #[test_matrix(
//     [
//         FixedVec::new(100_000),
//         SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [1_000, 10_000]
// )]
// fn con_grow_use_all<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_USERS, num_items_per_thread);
// }

// #[test_matrix(
//     [
//         FixedVec::new(2_000),
//         SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [250]
// )]
// fn miri_con_grow_use_all<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_USERS, num_items_per_thread);
// }
