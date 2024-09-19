// mod utils;
// use orx_concurrent_vec::*;
// use test_case::test_matrix;
// use utils::{elem, grow, sleep, Elem};

// const NUM_GROWERS: usize = 4;
// const NUM_USERS: usize = 4;
// const BATCH_SIZE: usize = 64;

// fn check_contains<P>(
//     vec: &ConcurrentVec<String, P>,
//     num_growers: usize,
//     num_items_per_thread: usize,
// ) where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let mut num_loops = 0;
//     let expected_len = num_growers * num_items_per_thread;
//     while vec.len() < expected_len || num_loops < 2 {
//         num_loops += 1;
//         for i in 0..num_growers {
//             for j in 0..num_items_per_thread {
//                 let might_exist = elem(i, j);
//                 let does_not_exist1 = elem(num_growers, 0);
//                 let does_not_exist2 = elem(i, num_items_per_thread);

//                 assert_eq!(vec.contains(&does_not_exist1), false);
//                 assert_eq!(vec.contains(&does_not_exist2), false);
//                 let _ = vec.contains(&might_exist);
//             }
//         }
//         sleep(true, 42);
//     }
// }

// fn spawn<P>(pinned: P, num_growers: usize, num_users: usize, num_items_per_thread: usize)
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

//         for _ in 0..num_users {
//             s.spawn(move || check_contains(con_vec, num_growers, num_items_per_thread));
//         }
//     });

//     assert_eq!(vec.len(), expected_len);
// }

// #[cfg(not(miri))]
// #[test_matrix(
//     [
//         // FixedVec::new(100_000),
//         // SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [1_000, 3_000]
// )]
// fn con_grow_contains<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_USERS, num_items_per_thread);
// }

// #[test_matrix(
//     [
//         // FixedVec::new(2_000),
//         // SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [100]
// )]
// fn miri_con_grow_contains<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_USERS, num_items_per_thread);
// }
