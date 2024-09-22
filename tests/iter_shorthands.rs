use orx_concurrent_vec::*;

#[test]
fn iter_map() {
    let vec = ConcurrentVec::new();
    vec.extend([0, 1, 2, 3, 4]);

    let doubles: Vec<_> = vec.map(|x| x * 2).collect();
    assert_eq!(&doubles, &[0, 2, 4, 6, 8]);
}

#[test]
fn iter_filter() {
    let vec = ConcurrentVec::new();
    vec.extend([0, 1, 2, 3, 4]);

    let doubles: Vec<_> = vec
        .filter(|x| x % 2 == 1)
        .map(|elem| elem.cloned())
        .collect();
    assert_eq!(&doubles, &[1, 3]);
}
