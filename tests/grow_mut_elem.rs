// mod utils;
// use orx_concurrent_vec::*;
// use test_case::test_matrix;
// use utils::{destruct_elem, elem, grow, select, sleep, Elem};

// const NUM_GROWERS: usize = 4;
// const NUM_MUTATORS: usize = 4;
// const BATCH_SIZE: usize = 64;

// // update | replace | set | swap
// const MUT_FREQ: [usize; 4] = [4, 4, 4, 4];

// fn replace<P>(
//     vec: &ConcurrentVec<String, P>,
//     index: usize,
//     num_growers: usize,
//     num_items_per_thread: usize,
// ) -> bool
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let thread_idx = index / 1_000_000;
//     let j = (index - 1_000_000 * thread_idx) % num_items_per_thread;
//     let new_value = elem(thread_idx, j);
//     let old_value = vec.replace(index, new_value.clone());
//     if let Some(old_value) = &old_value {
//         let (thread_idx, j) = destruct_elem(&old_value);
//         assert!(thread_idx < num_growers);
//         assert!(j < num_items_per_thread);
//     }
//     old_value.is_some()
// }

// fn set<P>(vec: &ConcurrentVec<String, P>, index: usize) -> bool
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let thread_idx = index / 1_000_000;
//     let j = index - 1_000_000 * thread_idx;
//     let value = elem(thread_idx, j);
//     vec.set(index, value.clone())
// }

// fn update<P>(
//     vec: &ConcurrentVec<String, P>,
//     index: usize,
//     num_growers: usize,
//     num_items_per_thread: usize,
// ) -> bool
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     vec.update(index, |x| {
//         let (thread_idx, j) = destruct_elem(&x);
//         assert!(thread_idx < num_growers);
//         assert!(j < num_items_per_thread);
//         let value = elem(thread_idx, j);
//         *x = value;
//     })
// }

// fn swap<P>(vec: &ConcurrentVec<String, P>, i: usize) -> bool
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let j = i / 10 + (i % 9);
//     vec.swap(i, j);
//     true
// }

// fn mut_element<P>(vec: &ConcurrentVec<String, P>, num_growers: usize, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     let expected_len = num_growers * num_items_per_thread;
//     while vec.len() < expected_len {
//         for i in 0..vec.len() {
//             let mutated = match select(&MUT_FREQ, i) {
//                 0 => update(vec, i, num_growers, num_items_per_thread),
//                 1 => replace(vec, i, num_growers, num_items_per_thread),
//                 2 => set(vec, i),
//                 _ => swap(vec, i),
//             };

//             if !mutated {
//                 break;
//             }
//         }
//         sleep(true, 42);
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
//             s.spawn(move || mut_element(con_vec, num_growers, num_items_per_thread));
//         }
//     });

//     assert_eq!(vec.len(), expected_len);
// }

// #[cfg(not(miri))]
// #[test_matrix(
//     [
//         SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         // FixedVec::new(15_000),
//         // SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [1_000, 3_000]
// )]
// fn con_grow_mut_elem<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_MUTATORS, num_items_per_thread);
// }

// #[test_matrix(
//     [
//         SplitVec::with_doubling_growth_and_fragments_capacity(32),
//         // FixedVec::new(2_000),
//         // SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
//     ],
//     [250]
// )]
// fn miri_con_grow_mut_elem<P>(pinned: P, num_items_per_thread: usize)
// where
//     P: IntoConcurrentPinnedVec<Elem> + Clone + 'static,
// {
//     spawn(pinned, NUM_GROWERS, NUM_MUTATORS, num_items_per_thread);
// }
