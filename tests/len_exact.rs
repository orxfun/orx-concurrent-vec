use orx_concurrent_vec::*;

#[test]
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
            loop {
                let exact_len = vec.len();
                let str = format!("len at {} = {}", i, exact_len);
                dbg!(str);
                std::thread::sleep(std::time::Duration::from_millis(1));

                match i {
                    i if (200..800).contains(&i) => assert_eq!(exact_len, 5),
                    _ => {}
                }

                i += 1;
                if exact_len == 10 {
                    break;
                }
            }
        });

        vec.extend(slow_iterator);
    });

    assert_eq!(vec.len(), 10);
}
