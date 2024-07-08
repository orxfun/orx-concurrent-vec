use orx_concurrent_vec::*;
use test_case::test_case;

fn run_test<P: PinnedVec<Option<String>> + Clone + 'static>(pinned: P) {
    let mut vec: ConcurrentVec<_, _> = pinned.into();
    for i in 0..2484 {
        vec.push(i.to_string());
    }

    for x in vec.iter_mut() {
        *x = format!("{}!", x);
    }

    *vec.get_mut(42).unwrap() = "x".to_string();
    *vec.get_mut(111).unwrap() = "y".to_string();
    *vec.get_mut(2242).unwrap() = "z".to_string();

    for (i, x) in vec.iter().enumerate() {
        let expected = match i {
            42 => "x".to_string(),
            111 => "y".to_string(),
            2242 => "z".to_string(),
            _ => format!("{}!", i),
        };
        assert_eq!(expected, x.as_str());
    }
}

#[test_case(FixedVec::new(2484))]
#[test_case(SplitVec::with_doubling_growth_and_fragments_capacity(32))]
#[test_case(SplitVec::with_recursive_growth_and_fragments_capacity(32))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(6, 8192))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(10, 512))]
#[test_case(SplitVec::with_linear_growth_and_fragments_capacity(14, 32))]
fn get_iter_mut<P: PinnedVec<Option<String>> + Clone + 'static>(pinned: P) {
    run_test(pinned)
}
