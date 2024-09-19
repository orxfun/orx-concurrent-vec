// mod utils;
// use orx_concurrent_vec::*;
// use test_case::test_matrix;
// use utils::{destruct_elem, elem, grow, sleep, Elem};

// const NUM_GROWERS: usize = 4;
// const NUM_MUTATORS: usize = 4;
// const BATCH_SIZE: usize = 4;

// fn replace<P>(vec: &ConcurrentVec<String, P>, num_growers: usize, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let old_values: Vec<_> = vec
//         .replace_all(|index| {
//             let thread_idx = index / 1_000_000;
//             let j = (index - 1_000_000 * thread_idx) % num_items_per_thread;
//             elem(thread_idx, j)
//         })
//         .collect();
//     for old_value in &old_values {
//         let (thread_idx, j) = destruct_elem(&old_value);
//         assert!(thread_idx < num_growers);
//         assert!(j < num_items_per_thread);
//     }
// }

// fn set<P>(vec: &ConcurrentVec<String, P>, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     vec.fill_with(|index| {
//         let thread_idx = index / 1_000_000;
//         let j = (index - 1_000_000 * thread_idx) % num_items_per_thread;
//         elem(thread_idx, j)
//     });
// }

// fn update<P>(vec: &ConcurrentVec<String, P>, num_growers: usize, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     vec.update_all(|x| {
//         let (thread_idx, j) = destruct_elem(&x);
//         assert!(thread_idx < num_growers);
//         assert!(j < num_items_per_thread);
//         let value = elem(thread_idx, j);
//         *x = value;
//     });
// }

// fn mut_all<P>(vec: &ConcurrentVec<String, P>, num_growers: usize, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let expected_len = num_growers * num_items_per_thread;
//     let mut mut_idx = 0;
//     while vec.len() < expected_len {
//         match mut_idx % 3 {
//             0 => update(vec, num_growers, num_items_per_thread),
//             1 => replace(vec, num_growers, num_items_per_thread),
//             _ => set(vec, num_items_per_thread),
//         };

//         sleep(mut_idx % 4 == 0, mut_idx);
//         mut_idx += 1;
//     }
// }

// fn spawn<P>(pinned: P, num_growers: usize, num_mutators: usize, num_items_per_thread: usize)
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

//         for _ in 0..num_mutators {
//             s.spawn(move || mut_all(con_vec, num_growers, num_items_per_thread));
//         }
//     });

//     assert_eq!(vec.len(), expected_len);
// }

// #[cfg(not(miri))]
// #[test_matrix(
//     [
//         FixedVec::new(15_000),
//         SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [1_000, 3_000]
// )]
// fn con_grow_mut_all<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_MUTATORS, num_items_per_thread);
// }

// #[test_matrix(
//     [
//         FixedVec::new(2_000),
//         SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [250]
// )]
// fn miri_con_grow_mut_all<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_MUTATORS, num_items_per_thread);
// }
