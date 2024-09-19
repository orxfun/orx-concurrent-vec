use orx_concurrent_option::ConcurrentOption;
use orx_concurrent_vec::*;
use std::time::Duration;
use test_case::test_matrix;

#[test]
fn capacity() {
    let mut split: SplitVec<_, Doubling> = (0..4).map(ConcurrentOption::from).collect();
    split.concurrent_reserve(5).expect("is-ok");
    let bag: ConcurrentVec<_, _> = split.into();
    assert_eq!(bag.capacity(), 4);
    bag.push(42);
    assert_eq!(bag.capacity(), 12);

    let mut split: SplitVec<_, Linear> = SplitVec::with_linear_growth(2);
    split.extend_from_slice(&[0, 1, 2, 3].map(ConcurrentOption::from));
    split.concurrent_reserve(5).expect("is-ok");
    let bag: ConcurrentVec<_, _> = split.into();
    assert_eq!(bag.capacity(), 4);
    bag.push(42);
    assert_eq!(bag.capacity(), 8);

    let mut fixed: FixedVec<_> = FixedVec::new(5);
    fixed.extend_from_slice(&[0, 1, 2, 3].map(ConcurrentOption::from));
    let bag: ConcurrentVec<_, _> = fixed.into();
    assert_eq!(bag.capacity(), 5);
    bag.push(42);
    assert_eq!(bag.capacity(), 5);
}

#[test_matrix([
    FixedVec::new(5),
    SplitVec::with_doubling_growth_and_fragments_capacity(1),
    SplitVec::with_linear_growth_and_fragments_capacity(2, 1)
])]
#[should_panic]
fn exceeding_fixed_capacity_panics<P: IntoConcurrentPinnedVec<ConcurrentOption<usize>>>(
    mut pinned_vec: P,
) {
    pinned_vec.clear();
    pinned_vec.extend_from_slice(&[0, 1, 2, 3].map(ConcurrentOption::from));
    let bag: ConcurrentVec<_, _> = pinned_vec.into();
    assert_eq!(bag.capacity(), 5);
    bag.push(42);
    bag.push(7);
}

#[test_matrix([
    FixedVec::new(10),
    SplitVec::with_doubling_growth_and_fragments_capacity(2),
    SplitVec::with_linear_growth_and_fragments_capacity(2, 3)
])]
#[should_panic]
fn exceeding_fixed_capacity_panics_concurrently<
    P: IntoConcurrentPinnedVec<ConcurrentOption<usize>>,
>(
    pinned_vec: P,
) {
    let bag: ConcurrentVec<_, _> = pinned_vec.into();
    let bag_ref = &bag;
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(move || {
                for _ in 0..4 {
                    // in total there will be 4*3 = 12 pushes
                    bag_ref.push(42);
                }
            });
        }
    });
}

#[test_matrix([
    FixedVec::new(100),
    SplitVec::with_doubling_growth_and_fragments_capacity(16),
    SplitVec::with_linear_growth_and_fragments_capacity(10, 16)
], [
    FixedVec::new(100),
    SplitVec::with_doubling_growth_and_fragments_capacity(16),
    SplitVec::with_linear_growth_and_fragments_capacity(10, 16)
])]
fn concurrent_get_and_iter<P, Q>(pinned_i32: P, pinned_f32: Q)
where
    P: IntoConcurrentPinnedVec<ConcurrentOption<i32>> + Clone,
    Q: IntoConcurrentPinnedVec<ConcurrentOption<f32>>,
{
    // record measurements in (assume) random intervals
    let measurements: ConcurrentVec<_, _> = pinned_i32.clone().into();
    let rf_measurements = &measurements;

    // collect sum of measurements every 50 milliseconds
    let sums: ConcurrentVec<_, _> = pinned_i32.into();
    let rf_sums = &sums;

    // collect average of measurements every 50 milliseconds
    let averages: ConcurrentVec<_, _> = pinned_f32.into();
    let rf_averages = &averages;

    std::thread::scope(|s| {
        s.spawn(move || {
            for i in 0..100 {
                std::thread::sleep(Duration::from_millis(i % 5));
                rf_measurements.push(i as i32);
            }
        });

        s.spawn(move || {
            for _ in 0..10 {
                let mut sum = 0;
                for i in 0..rf_measurements.len() {
                    sum += rf_measurements.get(i).copied().unwrap_or(0);
                }
                rf_sums.push(sum);
                std::thread::sleep(Duration::from_millis(50));
            }
        });

        s.spawn(move || {
            for _ in 0..10 {
                // better to have count & add as an atomic reduction for correctness
                // calling .len() and .iter().sum() separately might possibly give a false average
                fn count_and_add(len_and_sum: (usize, i32), x: &i32) -> (usize, i32) {
                    (len_and_sum.0 + 1, len_and_sum.1 + x)
                }

                let (len, sum) = rf_measurements.iter().fold((0, 0), count_and_add);
                let average = sum as f32 / len as f32;
                rf_averages.push(average);

                std::thread::sleep(Duration::from_millis(50));
            }
        });
    });

    assert_eq!(measurements.len(), 100);
    assert_eq!(sums.len(), 10);
    assert_eq!(averages.len(), 10);
}

#[test_matrix([
    FixedVec::new(10),
    SplitVec::with_doubling_growth_and_fragments_capacity(2),
    SplitVec::with_linear_growth_and_fragments_capacity(2, 3)
])]
fn get_mut<P: IntoConcurrentPinnedVec<ConcurrentOption<String>>>(pinned_vec: P) {
    let mut bag: ConcurrentVec<_, _> = pinned_vec.into();

    assert_eq!(None, bag.get_mut(0));

    bag.push("a".to_string());
    bag.push("b".to_string());
    bag.push("c".to_string());
    bag.push("d".to_string());
    bag.push("e".to_string());

    assert_eq!(None, bag.get_mut(5));
    assert_eq!(Some("c"), bag.get_mut(2).map(|x| x.as_str()));
    *bag.get_mut(2).unwrap() = "c!".to_string();

    let vec: Vec<_> = bag.iter().collect();
    assert_eq!(vec, ["a", "b", "c!", "d", "e"]);
}

#[test_matrix([
    FixedVec::new(10),
    SplitVec::with_doubling_growth_and_fragments_capacity(2),
    SplitVec::with_linear_growth_and_fragments_capacity(2, 3),
])]
fn iter_mut<P: IntoConcurrentPinnedVec<ConcurrentOption<String>>>(pinned_vec: P) {
    let mut bag: ConcurrentVec<_, _> = pinned_vec.into();

    assert_eq!(0, bag.iter_mut().count());

    bag.push("a".to_string());

    assert_eq!(Some("a"), bag.iter_mut().next().map(|x| x.as_str()));

    bag.push("b".to_string());
    bag.push("c".to_string());
    bag.push("d".to_string());
    bag.push("e".to_string());

    for x in bag.iter_mut().filter(|x| x.as_str() != "c") {
        *x = format!("{}!", x);
    }

    let vec: Vec<_> = bag.iter().collect();
    assert_eq!(vec, ["a!", "b!", "c", "d!", "e!"]);
}

#[test]
fn reserve_maximum_capacity() {
    // SplitVec<_, Doubling>
    let bag: ConcurrentVec<String> = ConcurrentVec::new();
    assert_eq!(bag.capacity(), 4); // only allocates the first fragment of 4
    assert_eq!(bag.maximum_capacity(), 17_179_869_180); // it can grow safely & exponentially

    let bag: ConcurrentVec<String, _> = ConcurrentVec::with_doubling_growth();
    assert_eq!(bag.capacity(), 4);
    assert_eq!(bag.maximum_capacity(), 17_179_869_180);

    // SplitVec<_, Linear>
    let mut bag: ConcurrentVec<String, _> = ConcurrentVec::with_linear_growth(10, 20);
    assert_eq!(bag.capacity(), 2usize.pow(10)); // only allocates first fragment of 1024
    assert_eq!(bag.maximum_capacity(), 2usize.pow(10) * 20); // it can concurrently allocate 19 more

    // SplitVec<_, Linear> -> reserve_maximum_capacity
    let new_max_capacity = bag.reserve_maximum_capacity(2usize.pow(10) * 30);
    assert!(new_max_capacity >= 2usize.pow(10) * 30);

    // actually no new allocation yet; precisely additional memory for 10 pairs of pointers is used
    assert_eq!(bag.capacity(), 2usize.pow(10)); // first fragment capacity

    assert!(bag.maximum_capacity() >= 2usize.pow(10) * 30); // now it can safely reach 2^10 * 30

    // FixedVec<_>
    let mut bag: ConcurrentVec<String, _> = ConcurrentVec::with_fixed_capacity(42);
    assert_eq!(bag.capacity(), 42);
    assert_eq!(bag.maximum_capacity(), 42);

    let new_max_capacity = bag.reserve_maximum_capacity(1024);
    assert!(new_max_capacity >= 1024);
}
