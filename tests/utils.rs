#![allow(dead_code)]

use orx_concurrent_vec::{ConcurrentElem, ConcurrentVec};
use orx_fixed_vec::IntoConcurrentPinnedVec;
use std::time::Duration;

pub type Elem = ConcurrentElem<String>;

pub fn elem(thread_idx: usize, j: usize) -> String {
    (thread_idx * 1_000_000 + j).to_string()
}

pub fn destruct_elem(value: &str) -> (usize, usize) {
    let x: usize = value.parse().unwrap();
    let thread_idx = x / 1_000_000;
    let j = x - 1_000_000 * thread_idx;
    (thread_idx, j)
}

pub fn sleep(do_sleep: bool, i: usize) {
    if do_sleep {
        let modulus = i % 3;
        let milliseconds = match modulus {
            0 => 0,
            1 => 10 + (i % 11) * 4,
            _ => 20 - (i % 5) * 3,
        } as u64;
        let duration = Duration::from_millis(milliseconds);
        std::thread::sleep(duration);
    }
}

pub fn grower<P>(vec: &ConcurrentVec<String, P>, num_items_to_add: usize, batch_size: Option<usize>)
where
    P: IntoConcurrentPinnedVec<Elem> + 'static,
{
    match batch_size {
        None => {
            for j in 0..num_items_to_add {
                let idx = vec.push("x".to_string());
                let old = vec[idx].replace(idx.to_string());
                assert_eq!(old.as_str(), "x");
                if j % 95 == 0 {
                    sleep(true, j);
                }
            }
        }
        Some(batch_size) => {
            let mut e = 0;
            let mut j = 0;
            while j < num_items_to_add {
                let extend_len = match num_items_to_add - j > batch_size {
                    true => batch_size,
                    false => num_items_to_add - j,
                };

                let iter = (j..(j + extend_len)).map(|_| "x".to_string());
                let begin_idx = vec.extend(iter);
                let slice = vec.slice(begin_idx..(begin_idx + extend_len));
                for (j, x) in slice.iter().enumerate() {
                    let idx = begin_idx + j;
                    let old = x.replace(idx.to_string());
                    assert_eq!(old, "x".to_string());
                }

                j += extend_len;

                e += 1;
                sleep(e % 2 == 0, j);
            }
        }
    }
}

pub fn select<const N: usize>(distribution: &[usize; N], index: usize) -> usize {
    let sum: usize = distribution.iter().sum();
    let m = index % sum;
    let mut partial_sum = 0;
    for (i, x) in distribution.iter().enumerate() {
        partial_sum += x;
        if m <= partial_sum {
            return i;
        }
    }
    distribution.len() - 1
}
