use orx_concurrent_vec::*;

#[test]
fn insert_at_start_middle_and_end() {
    let mut vec = ConcurrentVec::from_iter(['b', 'd']);

    vec.insert(0, 'a');
    vec.insert(2, 'c');
    vec.insert(4, 'e');

    assert_eq!(vec.len(), 5);
    assert_eq!(&vec, &['a', 'b', 'c', 'd', 'e']);
}

#[test]
fn insert_into_empty_and_after_pop() {
    let mut vec = ConcurrentVec::new();

    vec.insert(0, 1);
    vec.insert(1, 3);
    vec.insert(1, 2);

    assert_eq!(vec.pop(), Some(3));
    vec.insert(2, 4);

    assert_eq!(&vec, &[1, 2, 4]);
}

#[test]
#[should_panic(expected = "insertion index out of bounds")]
fn insert_out_of_bounds_panics() {
    let mut vec = ConcurrentVec::from_iter([1]);
    vec.insert(2, 3);
}
