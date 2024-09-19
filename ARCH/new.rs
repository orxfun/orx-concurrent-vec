use orx_concurrent_option::ConcurrentOption;
use orx_concurrent_vec::*;

#[test]
fn new_len_empty_clear() {
    fn test<P: IntoConcurrentPinnedVec<ConcurrentOption<char>>>(bag: ConcurrentVec<char, P>) {
        let mut bag = bag;

        assert!(bag.is_empty());
        assert_eq!(0, bag.len());

        bag.push('a');

        assert!(!bag.is_empty());
        assert_eq!(1, bag.len());

        bag.push('b');
        bag.push('c');
        bag.push('d');

        assert!(!bag.is_empty());
        assert_eq!(4, bag.len());

        bag.clear();
        assert!(bag.is_empty());
        assert_eq!(0, bag.len());
    }

    test(ConcurrentVec::new());
    test(ConcurrentVec::default());
    test(ConcurrentVec::with_doubling_growth());
    test(ConcurrentVec::with_linear_growth(2, 8));
    test(ConcurrentVec::with_linear_growth(4, 8));
    test(ConcurrentVec::with_fixed_capacity(64));
}
