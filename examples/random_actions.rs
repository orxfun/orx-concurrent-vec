use clap::Parser;
use orx_concurrent_vec::*;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::ops::Range;

enum ConAction {
    /// Pushes the string to the vec.
    Push(String),
    /// Extends the vec with the elements of the iterator.
    Extend(Vec<String>),
    /// Maps the string at the given index to its length.
    Map(usize),
    /// Clones the string at the given index of the vec.
    Clone(usize),
    /// Replaces the element at the given index with the new string.
    Replace(usize, String),
    /// Sets value of the element at the given index with the new string.
    Set(usize, String),
    /// Updates value of the element at the given index by appending the given character.
    Update(usize, char),
    /// Iterates over all elements, maps them to numbers and sums them.
    IterMapReduce,
    /// Iterates over the elements in the range and collects their clones.
    IterCloned(Range<usize>),
    /// Iterate over all elements, and remove the last character from the string
    /// if it is longer than one character.
    IterMutate,
}

impl ConAction {
    fn new(r: &mut ChaCha8Rng, vec_len: usize) -> Self {
        let idx = |r: &mut ChaCha8Rng| r.random_range(0..vec_len);
        let str = |r: &mut ChaCha8Rng| r.random_range(0..1000).to_string();

        if vec_len == 0 {
            // to avoid random_range panicking when vec_len = 0
            return ConAction::Push(str(r));
        }

        match r.random_range(0..10) {
            0 => ConAction::Push(str(r)),
            1 => ConAction::Extend((0..64).map(|_| str(r)).collect()),
            2 => ConAction::Map(idx(r)),
            3 => ConAction::Clone(idx(r)),
            4 => ConAction::Replace(idx(r), str(r)),
            5 => ConAction::Set(idx(r), str(r)),
            6 => ConAction::Update(idx(r), '7'),
            7 => ConAction::IterMapReduce,
            8 => {
                let i = idx(r);
                let j = r.random_range(i..vec_len);
                ConAction::IterCloned(i..j)
            }
            9 => ConAction::IterMutate,
            _ => panic!("?"),
        }
    }
}

/// Continuously applies random concurrent operations.
fn apply_random_concurrent_operations(
    vec: &ConcurrentVec<String>,
    final_vec_len: usize,
    mut r: ChaCha8Rng,
) {
    while vec.len() < final_vec_len {
        match ConAction::new(&mut r, vec.len()) {
            ConAction::Push(value) => {
                vec.push(value);
            }
            ConAction::Extend(iter) => {
                vec.extend(iter);
            }
            ConAction::Map(i) => {
                // when we know i is in-bounds
                let _num_chars = vec[i].map(|x: &String| x.len());
            }
            ConAction::Clone(i) => {
                let _clone: Option<String> = vec.get_cloned(i);
            }
            ConAction::Replace(i, new_value) => {
                // when we are not sure if i is in or out of bounds
                if let Some(elem) = vec.get(i) {
                    let old_value = elem.replace(new_value);
                    assert!(old_value.parse::<usize>().is_ok());
                }
            }
            ConAction::Set(i, new_value) => {
                // when we know i is in-bounds
                vec[i].set(new_value);
            }
            ConAction::Update(i, c) => {
                if let Some(elem) = vec.get(i) {
                    elem.update(|x| x.push(c));
                }
            }
            ConAction::IterMapReduce => {
                let sum: usize = vec
                    .iter()
                    .map(|elem| elem.map(|x| x.parse::<usize>().unwrap()))
                    .sum();
                assert!(sum > 0);
            }
            ConAction::IterCloned(range) => {
                let _collected: Vec<_> = vec.slice(range).iter_cloned().collect();
            }
            ConAction::IterMutate => {
                vec.iter().for_each(|elem| {
                    elem.update(|x: &mut String| {
                        if x.len() > 1 {
                            x.pop();
                        }
                    });
                });
            }
        }
    }
}

/// Program to test and demonstrate safety and convenience of
/// concurrent read and write operations with ConcurrentVec.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Number of threads to perform random concurrent actions.
    #[arg(long, default_value_t = 8)]
    num_threads: usize,

    /// Final length of the vector to build, read and update concurrently.
    #[arg(long, default_value_t = 1000)]
    len: usize,
}

fn main() {
    let args = Args::parse();

    assert!(args.num_threads > 0);
    assert!(args.len > 0);

    let vec = ConcurrentVec::new();

    std::thread::scope(|s| {
        let con_vec = &vec;
        for i in 0..args.num_threads {
            s.spawn(move || {
                apply_random_concurrent_operations(
                    &con_vec,
                    args.len,
                    ChaCha8Rng::seed_from_u64((i * 42) as u64),
                )
            });
        }
    });
}
