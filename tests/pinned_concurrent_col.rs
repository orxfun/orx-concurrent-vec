use orx_concurrent_vec::*;
use std::time::Duration;

#[test]
fn capacity() {
    let mut split: SplitVec<_, Doubling> = (0..4).map(Some).collect();
    split.concurrent_reserve(5).expect("is-ok");
    let bag: ConcurrentVec<_, _> = split.into();
    assert_eq!(bag.capacity(), 4);
    bag.push(42);
    assert_eq!(bag.capacity(), 12);

    let mut split: SplitVec<_, Recursive> = (0..4).map(Some).collect();
    split.concurrent_reserve(5).expect("is-ok");
    let bag: ConcurrentVec<_, _> = split.into();
    assert_eq!(bag.capacity(), 4);
    bag.push(42);
    assert_eq!(bag.capacity(), 12);

    let mut split: SplitVec<_, Linear> = SplitVec::with_linear_growth(2);
    split.extend_from_slice(&[0, 1, 2, 3].map(Some));
    split.concurrent_reserve(5).expect("is-ok");
    let bag: ConcurrentVec<_, _> = split.into();
    assert_eq!(bag.capacity(), 4);
    bag.push(42);
    assert_eq!(bag.capacity(), 8);

    let mut fixed: FixedVec<_> = FixedVec::new(5);
    fixed.extend_from_slice(&[0, 1, 2, 3].map(Some));
    let bag: ConcurrentVec<_, _> = fixed.into();
    assert_eq!(bag.capacity(), 5);
    bag.push(42);
    assert_eq!(bag.capacity(), 5);
}

#[test]
#[should_panic]
fn exceeding_fixed_capacity_panics() {
    let mut fixed: FixedVec<_> = FixedVec::new(5);
    fixed.extend_from_slice(&[0, 1, 2, 3].map(Some));
    let bag: ConcurrentVec<_, _> = fixed.into();
    assert_eq!(bag.capacity(), 5);
    bag.push(42);
    bag.push(7);
}

#[test]
#[should_panic]
fn exceeding_fixed_capacity_panics_concurrently() {
    let bag = ConcurrentVec::with_fixed_capacity(10);
    let bag_ref = &bag;
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(move || {
                for _ in 0..3 {
                    // in total there will be 4*3 = 12 pushes
                    bag_ref.push(42);
                }
            });
        }
    });
}

#[test]
fn get() {
    // record measurements in (assume) random intervals
    let measurements = ConcurrentVec::<i32>::new();
    let rf_measurements = &measurements;

    // collect sum of measurements every 50 milliseconds
    let sums = ConcurrentVec::new();
    let rf_sums = &sums;

    // collect average of measurements every 50 milliseconds
    let averages = ConcurrentVec::new();
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
                let count = rf_measurements.len();
                if count == 0 {
                    rf_averages.push(0.0);
                } else {
                    let mut sum = 0;
                    for i in 0..rf_measurements.len() {
                        sum += rf_measurements.get(i).copied().unwrap_or(0);
                    }
                    let average = sum as f32 / count as f32;
                    rf_averages.push(average);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    });

    assert_eq!(measurements.len(), 100);
    assert_eq!(sums.len(), 10);
    assert_eq!(averages.len(), 10);
}

#[test]
fn iter() {
    let mut bag = ConcurrentVec::new();

    assert_eq!(0, bag.iter().count());

    bag.push('a');

    assert_eq!(vec!['a'], bag.iter().copied().collect::<Vec<_>>());

    bag.push('b');
    bag.push('c');
    bag.push('d');

    assert_eq!(
        vec!['a', 'b', 'c', 'd'],
        bag.iter().copied().collect::<Vec<_>>()
    );

    bag.clear();
    assert_eq!(0, bag.iter().count());
}

#[test]
fn reserve_maximum_capacity() {
    // SplitVec<_, Doubling>
    let bag: ConcurrentVec<char> = ConcurrentVec::new();
    assert_eq!(bag.capacity(), 4); // only allocates the first fragment of 4
    assert_eq!(bag.maximum_capacity(), 17_179_869_180); // it can grow safely & exponentially

    let bag: ConcurrentVec<char, _> = ConcurrentVec::with_doubling_growth();
    assert_eq!(bag.capacity(), 4);
    assert_eq!(bag.maximum_capacity(), 17_179_869_180);

    // SplitVec<_, Linear>
    let mut bag: ConcurrentVec<char, _> = ConcurrentVec::with_linear_growth(10, 20);
    assert_eq!(bag.capacity(), 2usize.pow(10)); // only allocates first fragment of 1024
    assert_eq!(bag.maximum_capacity(), 2usize.pow(10) * 20); // it can concurrently allocate 19 more

    // SplitVec<_, Linear> -> reserve_maximum_capacity
    let result = bag.reserve_maximum_capacity(2usize.pow(10) * 30);
    assert!(result.is_ok());
    assert!(result.expect("is-ok") >= 2usize.pow(10) * 30);

    // actually no new allocation yet; precisely additional memory for 10 pairs of pointers is used
    assert_eq!(bag.capacity(), 2usize.pow(10)); // first fragment capacity

    assert!(bag.maximum_capacity() >= 2usize.pow(10) * 30); // now it can safely reach 2^10 * 30

    // FixedVec<_>: pre-allocated, exact and strict
    let mut bag: ConcurrentVec<char, _> = ConcurrentVec::with_fixed_capacity(42);
    assert_eq!(bag.capacity(), 42);
    assert_eq!(bag.maximum_capacity(), 42);

    let result = bag.reserve_maximum_capacity(43);
    assert!(result.is_err());
}
