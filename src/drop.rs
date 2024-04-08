use orx_concurrent_vec::*;
use test_case::test_matrix;

#[test_matrix(
    [
        FixedVec::new(100000),
        SplitVec::with_doubling_growth_and_fragments_capacity(32),
        SplitVec::with_recursive_growth_and_fragments_capacity(32),
        SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
    ],
    [124, 348, 1024, 2587, 42578]
)]
fn dropped_as_bag<P: PinnedVec<String>>(pinned_vec: P, len: usize) {
    let num_threads = 4;
    let num_items_per_thread = len / num_threads;

    let bag = fill_bag(pinned_vec, len);

    assert_eq!(bag.len(), num_threads * num_items_per_thread);
}

#[test_matrix(
    [
        FixedVec::new(100000),
        SplitVec::with_doubling_growth_and_fragments_capacity(32),
        SplitVec::with_recursive_growth_and_fragments_capacity(32),
        SplitVec::with_linear_growth_and_fragments_capacity(10, 64),
    ],
    [124, 348, 1024, 2587, 42578]
)]
fn dropped_after_into_inner<P: PinnedVec<String>>(pinned_vec: P, len: usize) {
    let num_threads = 4;
    let num_items_per_thread = len / num_threads;

    let bag = fill_bag(pinned_vec, len);

    let inner = bag.into_inner();
    assert_eq!(inner.len(), num_threads * num_items_per_thread);
}

fn fill_bag<P: PinnedVec<String>>(pinned_vec: P, len: usize) -> ConcurrentVec<String, P> {
    let num_threads = 4;
    let num_items_per_thread = len / num_threads;

    let bag: ConcurrentVec<_, _> = pinned_vec.into();
    let con_bag = &bag;
    std::thread::scope(move |s| {
        for _ in 0..num_threads {
            s.spawn(move || {
                for value in 0..num_items_per_thread {
                    let new_value = format!("from-thread-{}", value);
                    con_bag.push(new_value);
                }
            });
        }
    });

    bag
}
