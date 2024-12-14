use orx_concurrent_vec::*;

#[test]
#[cfg(not(miri))]
fn exact_len_with_slow_writer() {
    let vec = ConcurrentVec::<usize>::new();

    let slow_iterator = (0..10).map(|i| match i {
        5 => {
            std::thread::sleep(std::time::Duration::from_millis(1000));
            42
        }
        _ => {
            std::thread::sleep(std::time::Duration::from_millis(1));
            i
        }
    });

    std::thread::scope(|s| {
        let vec = &vec;
        // reader
        s.spawn(|| {
            let mut i = 0;
            let mut num_times_with_len5 = 0;
            loop {
                let exact_len = vec.len();
                let str = format!("len at {} = {}", i, exact_len);
                dbg!(str);
                std::thread::sleep(std::time::Duration::from_millis(1));

                num_times_with_len5 += match exact_len {
                    5 => 1,
                    _ => 0,
                };

                i += 1;
                if exact_len == 10 {
                    break;
                }
            }

            assert!(num_times_with_len5 > 100);
        });

        vec.extend(slow_iterator);
    });

    assert_eq!(vec.len(), 10);
}
