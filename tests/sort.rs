use orx_concurrent_vec::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn sort_orders_values() {
    let mut vec = ConcurrentVec::from_iter([3, 1, 2]);
    vec.sort();
    assert_eq!(&vec, &[1, 2, 3]);
}

#[test]
fn sort_by_orders_with_comparator() {
    let mut vec = ConcurrentVec::from_iter([1, 3, 2]);
    vec.sort_by(|a, b| b.cmp(a));
    assert_eq!(&vec, &[3, 2, 1]);
}

#[test]
fn sort_by_key_is_stable() {
    let mut vec = ConcurrentVec::from_iter([(2, 'a'), (1, 'b'), (2, 'c')]);
    vec.sort_by_key(|item| item.0);
    assert_eq!(&vec, &[(1, 'b'), (2, 'a'), (2, 'c')]);
}

#[test]
fn sort_applies_multiple_permutation_cycles() {
    let mut vec = ConcurrentVec::from_iter([2, 1, 0, 4, 3]);
    vec.sort();
    assert_eq!(&vec, &[0, 1, 2, 3, 4]);
}

#[test]
fn comparator_panic_leaves_vector_unchanged() {
    let mut vec = ConcurrentVec::from_iter([3, 1, 2]);
    let result = catch_unwind(AssertUnwindSafe(|| {
        vec.sort_by(|_, _| panic!("comparator panic"));
    }));

    assert!(result.is_err());
    assert_eq!(&vec, &[3, 1, 2]);
}

#[test]
fn key_panic_leaves_vector_unchanged() {
    let mut vec = ConcurrentVec::from_iter([3, 1, 2]);
    let result = catch_unwind(AssertUnwindSafe(|| {
        vec.sort_by_key(|_| -> usize { panic!("key panic") });
    }));

    assert!(result.is_err());
    assert_eq!(&vec, &[3, 1, 2]);
}
