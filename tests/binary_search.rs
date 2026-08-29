use orx_concurrent_vec::*;
use std::cmp::Ordering;

#[test]
fn binary_search_found_in_sorted_vec() {
    let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9]);
    assert_eq!(vec.binary_search(&5), Ok(2));
    assert_eq!(vec.binary_search(&1), Ok(0));
    assert_eq!(vec.binary_search(&9), Ok(4));
    assert_eq!(vec.binary_search(&3), Ok(1));
    assert_eq!(vec.binary_search(&7), Ok(3));
}

#[test]
fn binary_search_not_found() {
    let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9]);

    // Not found, should return insertion point
    assert_eq!(vec.binary_search(&0), Err(0));
    assert_eq!(vec.binary_search(&2), Err(1));
    assert_eq!(vec.binary_search(&4), Err(2));
    assert_eq!(vec.binary_search(&6), Err(3));
    assert_eq!(vec.binary_search(&8), Err(4));
    assert_eq!(vec.binary_search(&10), Err(5));
}

#[test]
fn binary_search_empty_vec() {
    let vec: ConcurrentVec<i32> = ConcurrentVec::new();
    assert_eq!(vec.binary_search(&42), Err(0));
}

#[test]
fn binary_search_single_element() {
    let vec = ConcurrentVec::from_iter([42]);
    assert_eq!(vec.binary_search(&42), Ok(0));
    assert_eq!(vec.binary_search(&41), Err(0));
    assert_eq!(vec.binary_search(&43), Err(1));
}

#[test]
fn binary_search_by_custom_comparator() {
    let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9]);

    // Search with custom comparator
    let result = vec.binary_search_by(|elem| {
        elem.map(|val| {
            if *val == 5 {
                Ordering::Equal
            } else if *val < 5 {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        })
    });
    assert_eq!(result, Ok(2));

    // Search for non-existent element
    let result = vec.binary_search_by(|elem| {
        elem.map(|val| {
            if *val == 6 {
                Ordering::Equal
            } else if *val < 6 {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        })
    });
    assert_eq!(result, Err(3));
}

#[test]
fn binary_search_by_key_simple() {
    #[derive(Clone)]
    struct Person {
        id: i32,
        #[allow(dead_code)]
        name: &'static str,
    }

    let vec = ConcurrentVec::from_iter(vec![
        Person {
            id: 1,
            name: "Alice",
        },
        Person { id: 3, name: "Bob" },
        Person {
            id: 5,
            name: "Charlie",
        },
        Person {
            id: 7,
            name: "David",
        },
    ]);

    // Found
    assert_eq!(
        vec.binary_search_by_key(&3, |elem| elem.map(|p| p.id)),
        Ok(1)
    );
    assert_eq!(
        vec.binary_search_by_key(&5, |elem| elem.map(|p| p.id)),
        Ok(2)
    );

    // Not found
    assert_eq!(
        vec.binary_search_by_key(&2, |elem| elem.map(|p| p.id)),
        Err(1)
    );
    assert_eq!(
        vec.binary_search_by_key(&6, |elem| elem.map(|p| p.id)),
        Err(3)
    );
}

#[test]
fn binary_search_slice_found() {
    let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9, 11, 13]);
    let slice = vec.slice(1..5); // [3, 5, 7, 9]

    assert_eq!(slice.binary_search(&5), Ok(1));
    assert_eq!(slice.binary_search(&7), Ok(2));
    assert_eq!(slice.binary_search(&3), Ok(0));
}

#[test]
fn binary_search_slice_not_found() {
    let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9, 11, 13]);
    let slice = vec.slice(1..5); // [3, 5, 7, 9]

    assert_eq!(slice.binary_search(&4), Err(1));
    assert_eq!(slice.binary_search(&6), Err(2));
    assert_eq!(slice.binary_search(&10), Err(4));
}

#[test]
fn binary_search_by_on_slice() {
    let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9]);
    let slice = vec.as_slice();

    let result = slice.binary_search_by(|elem| {
        elem.map(|val| {
            if *val == 5 {
                Ordering::Equal
            } else if *val < 5 {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        })
    });
    assert_eq!(result, Ok(2));
}

#[test]
fn binary_search_with_duplicates() {
    let vec = ConcurrentVec::from_iter([1, 2, 2, 2, 3, 4, 5]);

    // When duplicates exist, binary_search finds one of them
    let result = vec.binary_search(&2);
    assert!(result.is_ok());
    let idx = result.unwrap();
    assert!(idx >= 1 && idx <= 3);
}

#[test]
fn binary_search_strings() {
    let vec = ConcurrentVec::from_iter(vec!["apple", "banana", "cherry", "date", "elderberry"]);

    assert_eq!(vec.binary_search(&"cherry"), Ok(2));
    assert_eq!(vec.binary_search(&"date"), Ok(3));
    assert_eq!(vec.binary_search(&"apricot"), Err(1));
    assert_eq!(vec.binary_search(&"fig"), Err(5));
}

#[test]
fn binary_search_concurrent_safe() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let vec = Arc::new(ConcurrentVec::from_iter([1, 3, 5, 7, 9]));
    let barrier = Arc::new(Barrier::new(3));

    let handles: Vec<_> = (0..3)
        .map(|_| {
            let vec = Arc::clone(&vec);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait(); // Ensure all threads start at the same time

                // Perform multiple binary searches concurrently
                for i in 0..100 {
                    let search_val = (i % 5) * 2 + 1; // Search for 1, 3, 5, 7, 9
                    let result = vec.binary_search(&search_val);
                    assert!(result.is_ok(), "Failed to find {}", search_val);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn binary_search_with_concurrent_growth() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    // Create a vec and grow it while performing searches
    // The search should only consider elements that existed at the start
    let vec = Arc::new(ConcurrentVec::from_iter([1, 3, 5, 7, 9]));
    let barrier = Arc::new(Barrier::new(2));

    let vec_reader = Arc::clone(&vec);
    let barrier_reader = Arc::clone(&barrier);

    let reader = thread::spawn(move || {
        barrier_reader.wait();

        // Search should find elements that were present at start
        let result = vec_reader.binary_search(&5);
        assert_eq!(result, Ok(2));
    });

    let vec_writer = Arc::clone(&vec);
    let barrier_writer = Arc::clone(&barrier);

    let writer = thread::spawn(move || {
        barrier_writer.wait();

        // Add more elements (these may or may not be seen by the search)
        vec_writer.extend([11, 13, 15]);
    });

    reader.join().unwrap();
    writer.join().unwrap();

    // Verify the vec has grown
    assert!(vec.len() >= 5);
}
