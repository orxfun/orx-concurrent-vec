use orx_concurrent_vec::*;

#[test]
fn remove_at_start_middle_and_end() {
    let mut vec = ConcurrentVec::from_iter([1, 2, 3, 4, 5]);

    assert_eq!(vec.remove(2), 3);
    assert_eq!(vec.remove(0), 1);
    assert_eq!(vec.remove(2), 5);

    assert_eq!(vec.len(), 2);
    assert_eq!(&vec, &[2, 4]);
}

#[test]
fn remove_last_and_reuse_slot() {
    let mut vec = ConcurrentVec::from_iter([1, 2]);

    assert_eq!(vec.remove(1), 2);
    vec.push(3);

    assert_eq!(vec.remove(0), 1);
    assert_eq!(&vec, &[3]);
}

#[test]
#[should_panic(expected = "removal index out of bounds")]
fn remove_out_of_bounds_panics() {
    let mut vec = ConcurrentVec::from_iter([1]);
    vec.remove(1);
}
