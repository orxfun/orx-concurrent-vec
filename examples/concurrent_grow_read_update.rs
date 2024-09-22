use clap::Parser;
use orx_concurrent_vec::*;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

/// Randomly draws an element from the `candidates.`
fn draw<'a, T>(r: &mut ChaCha8Rng, candidates: &'a [T]) -> &'a T {
    &candidates[r.gen_range(0..candidates.len())]
}

/// Note that [`vec.push(value)`] could also be used, which returns the position
/// or idx that the value is written to.
///
/// However, for test purposes, we want the i-th position of the vec to be
/// i.to_string(), so that we can assert.
///
/// Therefore, we use [`push_for_idx`] which creates the value to be pushed
/// after its idx is reserved. This allows, us to make sure that the i-th
/// element will be equal to i.to_string().
///
/// [`vec.push(value)`]: crate::ConcurrentVec::push
/// [`push_for_idx`]: crate::ConcurrentVec::push_for_idx
fn push(vec: &ConcurrentVec<String>, num_items_to_add: usize, lag: u64) {
    for _ in 0..num_items_to_add {
        std::thread::sleep(std::time::Duration::from_micros(lag));

        vec.push_for_idx(|idx| idx.to_string());
    }
}

/// Note that [`vec.extend(into_iter)`] could also be used, which returns the position
/// or idx of position that the first value of the iterator is written to,
/// and the remaining elements are written sequentially.
///
/// However, for test purposes, we want the i-th position of the vec to be
/// i.to_string(), so that we can assert.
///
/// Therefore, we use [`extend_for_idx`] which creates the value to be pushed
/// after its idx is reserved. This allows, us to make sure that the i-th
/// element will be equal to i.to_string().
///
/// [`vec.extend(into_iter)`]: crate::ConcurrentVec::extend
/// [`extend_for_idx`]: crate::ConcurrentVec::extend_for_idx
fn extend(vec: &ConcurrentVec<String>, num_items_to_add: usize, batch_size: usize, lag: u64) {
    let mut num_added = 0;
    while num_added < num_items_to_add {
        std::thread::sleep(std::time::Duration::from_micros(lag));

        let extend_len = match num_items_to_add - num_added > batch_size {
            true => batch_size,
            false => num_items_to_add - num_added,
        };

        let iter = |begin_idx: usize| (begin_idx..(begin_idx + extend_len)).map(|j| j.to_string());
        vec.extend_for_idx(iter, extend_len);

        num_added += extend_len;
    }
}

/// We concurrently read the elements while they are being added and updated concurrently.
///
/// Note that ConcurrentVec does not provide out `&T` or `&mut T` in its safe api.
/// * [`vec.get(i)`] or `vec[i]` returns a `ConcurrentElement` rather than directly the reference.
/// * `ConcurrentElement` provides safe methods to use the underlying data.
///
/// This function demonstrates three thread-safe ways to access the data without mutation:
/// * [`elem.map`] allows to map an element by applying a function on its value, where `elem = vec[i]`,
/// * [`vec.get_cloned(i)`] reads the value of an element and returns a clone of it.
/// * lastly, a thread-safe equality check is demonstrated.
///
/// [`vec.get(i)`]: crate::ConcurrentVec::get
/// [`elem.map`]: crate::ConcurrentElement::map
/// [`vec.get_cloned(i)`]: crate::ConcurrentVec::get_cloned
fn read(vec: &ConcurrentVec<String>, final_len: usize, lag: u64) {
    enum Read {
        Map,
        GetCloned,
        Eq,
    }
    const READ_OPS: [Read; 3] = [Read::Map, Read::GetCloned, Read::Eq];

    let mut rng = ChaCha8Rng::seed_from_u64(5648);

    while vec.len() < final_len {
        let slice = vec.as_slice();
        for _ in 0..1000 {
            std::thread::sleep(std::time::Duration::from_micros(lag));

            let idx = rng.gen_range(0..slice.len());

            match draw(&mut rng, &READ_OPS) {
                Read::Map => {
                    let mapped = slice[idx].map(|x| format!("{}!", x));
                    assert_eq!(mapped, format!("{}!", idx));
                }
                Read::GetCloned => {
                    let clone: Option<String> = slice.get_cloned(idx);
                    assert_eq!(clone, Some(idx.to_string()));
                }
                Read::Eq => {
                    assert_eq!(slice[idx], idx.to_string())
                }
            }
        }
    }
}

/// We concurrently iterate over elements and sum up their values.
///
/// Note that ConcurrentVec does not provide out `&T` or `&mut T` in its safe api.
/// * [`vec.iter()`] returns an iterator of `ConcurrentElement`s rather than direct references.
/// * [`ConcurrentElement`] provides safe methods to access the underlying data.
/// * Further, it allows to mutate the data; and hence, a separate `iter_mut` method is not needed.
///
/// [`vec.iter()`]: crate::ConcurrentVec::iter
fn iterate(vec: &ConcurrentVec<String>, final_len: usize) {
    while vec.len() < final_len {
        let slice = vec.as_slice();

        let sum1: usize = slice
            .iter()
            .map(|elem| elem.map(|x| x.parse::<usize>().unwrap()))
            .sum();

        let sum2: usize = slice
            .iter_cloned()
            .map(|x| x.parse::<usize>().unwrap())
            .sum();

        assert_eq!(sum1, sum2);
    }

    let final_sum: usize = vec
        .iter()
        .map(|elem| elem.map(|x| x.parse::<usize>().unwrap()))
        .sum();
    let expected_final_sum = final_len * (final_len - 1) / 2;

    assert_eq!(final_sum, expected_final_sum);
}

/// We concurrently update the elements while they are being added and updated concurrently.
///
/// Note that ConcurrentVec does not provide out `&T` or `&mut T` in its safe api.
/// * `vec.get(i)` or `vec[i]` returns a `ConcurrentElement` rather than directly the reference.
/// * `ConcurrentElement` provides safe methods to use the underlying data.
///
/// This function demonstrates three thread-safe ways to mutate the data:
/// * `update` allows to change an element based on its previous value,
///   it can be considered as the concurrent counterpart of `get_mut`.
/// * `set` overwrites the element with the given value.
/// * `replace` is similar to `set` except that it additionally returns the
///   prior value.
fn update(vec: &ConcurrentVec<String>, final_len: usize, lag: u64) {
    enum Update {
        Update,
        Set,
        Replace,
    }

    const UPDATE_OPS: [Update; 3] = [Update::Update, Update::Set, Update::Replace];

    let mut rng = ChaCha8Rng::seed_from_u64(47);

    while vec.len() < final_len {
        let slice = vec.as_slice();
        for _ in 0..1000 {
            std::thread::sleep(std::time::Duration::from_micros(lag));

            let idx = rng.gen_range(0..slice.len());

            match draw(&mut rng, &UPDATE_OPS) {
                Update::Update => {
                    slice[idx].update(|x| {
                        let number: usize = x.parse().unwrap();
                        *x = number.to_string();
                    });
                }
                Update::Set => slice[idx].set(idx.to_string()),
                Update::Replace => {
                    let new_value = idx.to_string();
                    let old_value = slice[idx].replace(new_value);
                    assert_eq!(old_value, idx.to_string());
                }
            }
        }
    }
}

/// Program to test and demonstrate safety and convenience of
/// concurrent read and write operations with ConcurrentVec.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Amount of lag in microseconds to add between each operation,
    /// such as each 'push' or each 'update' etc.
    #[arg(long, default_value_t = 0)]
    lag: u64,

    /// Final length of the vector to build, read and update concurrently.
    #[arg(long, default_value_t = 1000)]
    len: usize,

    /// Number of threads that will push elements one by one.
    #[arg(long, default_value_t = 2)]
    num_pushers: usize,

    /// Number of threads that will extend the vec by batches of 64 elements.
    #[arg(long, default_value_t = 2)]
    num_extenders: usize,

    /// Number of threads that will continuously read elements.
    #[arg(long, default_value_t = 2)]
    num_readers: usize,

    /// Number of threads that will continuously update elements.
    #[arg(long, default_value_t = 2)]
    num_updaters: usize,

    /// Number of threads that will continuously iterate over elements.
    #[arg(long, default_value_t = 2)]
    num_iterators: usize,
}

fn main() {
    let args = Args::parse();

    assert!(args.num_pushers + args.num_extenders > 0);

    let num_items_per_thread = args.len / (args.num_pushers + args.num_extenders);
    let final_len = num_items_per_thread * (args.num_pushers + args.num_extenders);

    let con_vec = ConcurrentVec::new();

    std::thread::scope(|s| {
        for _ in 0..args.num_pushers {
            s.spawn(|| push(&con_vec, num_items_per_thread, args.lag));
        }

        for _ in 0..args.num_extenders {
            s.spawn(|| extend(&con_vec, num_items_per_thread, 64, args.lag));
        }

        for _ in 0..args.num_readers {
            s.spawn(|| read(&con_vec, final_len, args.lag));
        }

        for _ in 0..args.num_updaters {
            s.spawn(|| update(&con_vec, final_len, args.lag));
        }

        for _ in 0..args.num_iterators {
            s.spawn(|| iterate(&con_vec, final_len));
        }
    });

    assert_eq!(con_vec.len(), final_len);

    // convert into "non-concurrent" vec
    let vec = con_vec.into_inner();
    for (i, x) in vec.iter().enumerate() {
        assert_eq!(x, &i.to_string());
    }
}
