use clap::Parser;
use orx_concurrent_vec::*;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

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

/// We concurrently read the elements while they are being added.
///
/// Note that ConcurrentVec does not provide out `&T` or `&mut T` in its safe api.
/// * [`vec.get(i)`] or `vec[i]` returns a `ConcurrentElement` rather than directly the reference.
/// * `ConcurrentElement` provides safe methods to use the underlying data.
///
/// However, we can have direct access using `unsafe` methods such as:
/// * [`get_ref(i)`] -> as demonstrated in this function
/// * [`iter_ref()`] which yields `&T`
/// * [`get_mut(i)`] that returns `Option<&mut T>`
/// * [`iter_mut()`] which yields `&mut T`
///
/// # SAFETY
///
/// To be able to safely use these methods, the caller must guarantee that there will
/// never be a concurrent read & update access.
///
/// Here, update access refers to methods which change the value of an existing element:
/// * using thread-safe methods such as `update`, `set` or `replace`, or
/// * using the unsafe methods `get_mut` or `iter_mut`.
///
/// Note that update methods are to be concerned about; however, growth methods are not.
/// `ConcurrentVec` is able to limit access to elements until they are completely initialized;
/// and hence, no need to worry about concurrent `push` and `extend` calls.
///
/// In this scenario, we concurrently push to and extend the vector, while by other threads
/// we read the elements. No updates happening. Therefore:
/// * instead of the [`ConcurrentElement`] methods providing thread-safe read access such as
///   [`map`] or [`clone`];
/// * we can use `get_ref` to directly access the value (`&T`) of the element.
///
/// [`vec.get(i)`]: crate::ConcurrentVec::get
/// [`get_ref(i)`]: crate::ConcurrentVec::get_ref
/// [`iter_ref()`]: crate::ConcurrentVec::iter_ref
/// [`get_mut(i)`]: crate::ConcurrentVec::get_mut
/// [`iter_mut()`]: crate::ConcurrentVec::iter_mut
/// [`map`]: crate::ConcurrentElement::map
/// [`clone`]: crate::ConcurrentElement::clone
fn read(vec: &ConcurrentVec<String>, final_len: usize, lag: u64) {
    let mut rng = ChaCha8Rng::seed_from_u64(312);

    while vec.len() < final_len {
        let slice = vec.as_slice();

        if slice.is_empty() {
            // to prevent get_range from panicking
            continue;
        }

        for _ in 0..1000 {
            std::thread::sleep(std::time::Duration::from_micros(lag));

            let idx = rng.gen_range(0..slice.len());

            // direct access to the reference
            // SAFETY: safe in this scenario since we know that no update on the element happens.
            let value = unsafe { slice.get_ref(idx) }.unwrap();

            assert_eq!(value, &idx.to_string());
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
/// However, we can have direct access using `unsafe` methods such as:
/// * [`get_ref(i)`] that returns `Option<&T>`
/// * [`iter_ref()`] which yields `&T` as demonstrated in this function
/// * [`get_mut(i)`] that returns `Option<&mut T>`
/// * [`iter_mut()`] which yields `&mut T`
///
/// # SAFETY
///
/// To be able to safely use these methods, the caller must guarantee that there will
/// never be a concurrent read & update access.
///
/// Here, update access refers to methods which change the value of an existing element:
/// * using thread-safe methods such as `update`, `set` or `replace`, or
/// * using the unsafe methods `get_mut` or `iter_mut`.
///
/// Note that update methods are to be concerned about; however, growth methods are not.
/// `ConcurrentVec` is able to limit access to elements until they are completely initialized;
/// and hence, no need to worry about concurrent `push` and `extend` calls.
///
/// In this scenario, we concurrently push to and extend the vector, while by other threads
/// we read the elements. No updates happening. Therefore:
/// * instead of the [`ConcurrentElement`] methods providing thread-safe read access such as
///   [`map`] or [`clone`];
/// * we can use `iter_ref` to directly access the values (`&T`) of elements.
///
/// [`vec.iter()`]: crate::ConcurrentVec::iter
/// [`get_ref(i)`]: crate::ConcurrentVec::get_ref
/// [`iter_ref()`]: crate::ConcurrentVec::iter_ref
/// [`get_mut(i)`]: crate::ConcurrentVec::get_mut
/// [`iter_mut()`]: crate::ConcurrentVec::iter_mut
fn iterate(vec: &ConcurrentVec<String>, final_len: usize) {
    while vec.len() < final_len {
        let slice = vec.as_slice();

        let _sum: usize = unsafe { slice.iter_ref() }
            .map(|x| x.parse::<usize>().unwrap())
            .sum();
    }

    let final_sum: usize = unsafe { vec.iter_ref() }
        .map(|x| x.parse::<usize>().unwrap())
        .sum();
    let expected_final_sum = final_len * (final_len - 1) / 2;

    assert_eq!(final_sum, expected_final_sum);
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

    /// Number of threads that will continuously iterate over elements.
    #[arg(long, default_value_t = 0)]
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
