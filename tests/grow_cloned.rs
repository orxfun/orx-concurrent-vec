// mod utils;
// use orx_concurrent_vec::*;
// use test_case::test_matrix;
// use utils::{grow, sleep, Elem};

// const NUM_GROWERS: usize = 4;
// const NUM_CLONERS: usize = 4;
// const BATCH_SIZE: usize = 64;

// fn use_single_element(value: &String, num_growers: usize, num_items_per_thread: usize) {
//     let x: usize = value.parse().unwrap();
//     let thread_idx = x / 1_000_000;
//     let j = x - 1_000_000 * thread_idx;
//     assert!(thread_idx < num_growers);
//     assert!(j < num_items_per_thread);
// }

// fn clone<P>(
//     vec: &ConcurrentVec<String, P>,
//     thread_idx: usize,
//     num_growers: usize,
//     num_items_per_thread: usize,
// ) where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let expected_len = num_growers * num_items_per_thread;
//     while vec.len() < expected_len {
//         match thread_idx % 2 == 0 {
//             true => {
//                 for i in 0..vec.len() {
//                     if let Some(value) = vec.cloned(i) {
//                         use_single_element(&value, num_growers, num_items_per_thread);
//                     }
//                 }
//                 sleep(true, 42);
//             }
//             false => {
//                 let cloned = vec.iter_cloned();
//                 for value in cloned {
//                     use_single_element(&value, num_growers, num_items_per_thread);
//                 }
//             }
//         }
//     }
// }

// fn spawn<P>(pinned: P, num_growers: usize, num_cloners: usize, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let vec: ConcurrentVec<_, _> = pinned.clone().into();
//     let con_vec = &vec;
//     let expected_len = num_growers * num_items_per_thread;

//     std::thread::scope(|s| {
//         for i in 0..num_growers {
//             s.spawn(move || grow(con_vec, i, num_items_per_thread, BATCH_SIZE));
//         }

//         for i in 0..num_cloners {
//             s.spawn(move || clone(con_vec, i, num_growers, num_items_per_thread));
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
// fn con_grow_clone<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_CLONERS, num_items_per_thread);
// }

// #[test_matrix(
//     [
//         FixedVec::new(2_000),
//         SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [250]
// )]
// fn miri_con_grow_clone<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_CLONERS, num_items_per_thread);
// }
