use orx_concurrent_vec::*;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

#[test]
fn into_inner_from() {
    let bag = ConcurrentVec::new();

    bag.push('a');
    bag.push('b');
    bag.push('c');
    bag.push('d');
    assert_eq!(
        vec!['a', 'b', 'c', 'd'],
        bag.iter().copied().collect::<Vec<_>>()
    );

    let mut split = bag.into_inner();
    assert_eq!(
        vec!['a', 'b', 'c', 'd'],
        split.iter().copied().flatten().collect::<Vec<_>>()
    );

    split.push(Some('e'));
    *split.get_mut(0).expect("exists") = Some('x');

    assert_eq!(
        vec!['x', 'b', 'c', 'd', 'e'],
        split.iter().copied().flatten().collect::<Vec<_>>()
    );

    let mut bag: ConcurrentVec<_> = split.into();
    assert_eq!(
        vec!['x', 'b', 'c', 'd', 'e'],
        bag.iter().copied().collect::<Vec<_>>()
    );

    bag.clear();
    assert!(bag.is_empty());

    let split = bag.into_inner();
    assert!(split.is_empty());
}

#[test]
fn ok_at_num_threads() {
    use std::thread::available_parallelism;
    let default_parallelism_approx = available_parallelism().expect("is-ok").get();

    let num_threads = default_parallelism_approx;
    let num_items_per_thread = 16384;

    let bag = ConcurrentVec::new();
    let bag_ref = &bag;
    std::thread::scope(|s| {
        for i in 0..num_threads {
            s.spawn(move || {
                for j in 0..num_items_per_thread {
                    bag_ref.push((i * 100000 + j) as i32);
                }
            });
        }
    });

    let pinned = bag.into_inner();
    assert_eq!(pinned.len(), num_threads * num_items_per_thread);
}

#[test]
fn push_indices() {
    let num_threads = 4;
    let num_items_per_thread = 16;

    let indices_set = Arc::new(Mutex::new(HashSet::new()));

    let bag = ConcurrentVec::new();
    let bag_ref = &bag;
    std::thread::scope(|s| {
        for i in 0..num_threads {
            let indices_set = indices_set.clone();
            s.spawn(move || {
                for j in 0..num_items_per_thread {
                    let idx = bag_ref.push(i * 100000 + j);
                    let mut set = indices_set.lock().expect("is ok");
                    set.insert(idx);
                }
            });
        }
    });

    let set = indices_set.lock().expect("is ok");
    assert_eq!(set.len(), 4 * 16);
    for i in 0..(4 * 16) {
        assert!(set.contains(&i));
    }
}

#[test]
fn extend_indices() {
    let num_threads = 4;
    let num_items_per_thread = 16;

    let indices_set = Arc::new(Mutex::new(HashSet::new()));

    let bag = ConcurrentVec::new();
    let bag_ref = &bag;
    std::thread::scope(|s| {
        for i in 0..num_threads {
            let indices_set = indices_set.clone();
            s.spawn(move || {
                let iter = (0..num_items_per_thread).map(|j| i * 100000 + j);

                let begin_idx = bag_ref.extend(iter);

                let mut set = indices_set.lock().expect("is ok");
                set.insert(begin_idx);
            });
        }
    });

    let set = indices_set.lock().expect("is ok");
    assert_eq!(set.len(), num_threads);
    for i in 0..num_threads {
        assert!(set.contains(&(i * num_items_per_thread)));
    }
}

#[test]
fn extend_n_items_indices() {
    let num_threads = 4;
    let num_items_per_thread = 16;

    let indices_set = Arc::new(Mutex::new(HashSet::new()));

    let bag = ConcurrentVec::new();
    let bag_ref = &bag;
    std::thread::scope(|s| {
        for i in 0..num_threads {
            let indices_set = indices_set.clone();
            s.spawn(move || {
                let iter = (0..num_items_per_thread).map(|j| i * 100000 + j);

                let begin_idx = unsafe { bag_ref.extend_n_items(iter, num_items_per_thread) };

                let mut set = indices_set.lock().expect("is ok");
                set.insert(begin_idx);
            });
        }
    });

    let set = indices_set.lock().expect("is ok");
    assert_eq!(set.len(), num_threads);
    for i in 0..num_threads {
        assert!(set.contains(&(i * num_items_per_thread)));
    }
}
