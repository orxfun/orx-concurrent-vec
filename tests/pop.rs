use orx_concurrent_vec::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn pop_empty() {
    let mut vec: ConcurrentVec<i32> = ConcurrentVec::new();
    assert_eq!(vec.pop(), None);
    assert_eq!(vec.len(), 0);
    assert!(vec.is_empty());
}

#[test]
fn pop_basic() {
    let mut vec = ConcurrentVec::new();
    vec.push(10);
    vec.push(20);
    vec.push(30);

    assert_eq!(vec.len(), 3);
    assert_eq!(vec.pop(), Some(30));
    assert_eq!(vec.len(), 2);
    assert_eq!(vec.pop(), Some(20));
    assert_eq!(vec.len(), 1);
    assert_eq!(vec.pop(), Some(10));
    assert_eq!(vec.len(), 0);
    assert_eq!(vec.pop(), None);
    assert_eq!(vec.len(), 0);
    assert!(vec.is_empty());
}

#[test]
fn pop_and_push_interleaved() {
    let mut vec = ConcurrentVec::new();

    vec.push(1);
    vec.push(2);
    assert_eq!(vec.pop(), Some(2));

    vec.push(3);
    vec.push(4);
    assert_eq!(vec.pop(), Some(4));
    assert_eq!(vec.pop(), Some(3));
    assert_eq!(vec.pop(), Some(1));
    assert_eq!(vec.pop(), None);

    vec.push(100);
    assert_eq!(vec.len(), 1);
    assert_eq!(vec.pop(), Some(100));
    assert_eq!(vec.pop(), None);
}

#[test]
fn pop_and_extend_interleaved() {
    let mut vec = ConcurrentVec::new();

    vec.extend(0..10);
    assert_eq!(vec.len(), 10);

    for i in (5..10).rev() {
        assert_eq!(vec.pop(), Some(i));
    }
    assert_eq!(vec.len(), 5);

    vec.extend(10..15);
    assert_eq!(vec.len(), 10);

    let result: Vec<_> = vec.iter().map(|e| e.copied()).collect();
    assert_eq!(result, vec![0, 1, 2, 3, 4, 10, 11, 12, 13, 14]);

    let into_res: Vec<_> = vec.into_iter().collect();
    assert_eq!(into_res, vec![0, 1, 2, 3, 4, 10, 11, 12, 13, 14]);
}

#[test]
fn pop_with_conversions() {
    let mut vec = ConcurrentVec::new();
    vec.extend(0..5);

    assert_eq!(vec.pop(), Some(4));
    assert_eq!(vec.pop(), Some(3));

    assert_eq!(vec.to_vec(), vec![0, 1, 2]);
}

#[test]
fn pop_drop_tracker() {
    #[derive(Clone)]
    struct Dropper {
        _id: usize,
        drop_count: Arc<AtomicUsize>,
    }

    impl Drop for Dropper {
        fn drop(&mut self) {
            self.drop_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    let drop_count = Arc::new(AtomicUsize::new(0));

    {
        let mut vec = ConcurrentVec::new();
        for i in 0..10 {
            vec.push(Dropper {
                _id: i,
                drop_count: drop_count.clone(),
            });
        }

        // Pop 3 elements and immediately drop them
        for _ in 0..3 {
            let popped = vec.pop();
            assert!(popped.is_some());
            drop(popped);
        }

        assert_eq!(drop_count.load(Ordering::Relaxed), 3);
        assert_eq!(vec.len(), 7);

        // Vector goes out of scope here; remaining 7 elements should be dropped
    }

    assert_eq!(drop_count.load(Ordering::Relaxed), 10);
}

#[test]
fn pop_after_concurrent_work() {
    let mut vec = ConcurrentVec::new();

    std::thread::scope(|s| {
        let vec_ref = &vec;
        for t in 0..4 {
            s.spawn(move || {
                for i in 0..100 {
                    vec_ref.push(t * 1000 + i);
                }
            });
        }
    });

    assert_eq!(vec.len(), 400);

    for _ in 0..50 {
        assert!(vec.pop().is_some());
    }

    assert_eq!(vec.len(), 350);

    vec.push(999_999);
    assert_eq!(vec.len(), 351);
    assert_eq!(vec.pop(), Some(999_999));
    assert_eq!(vec.len(), 350);
}
