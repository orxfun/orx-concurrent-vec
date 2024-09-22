# orx-concurrent-vec

[![orx-concurrent-vec crate](https://img.shields.io/crates/v/orx-concurrent-vec.svg)](https://crates.io/crates/orx-concurrent-vec)
[![orx-concurrent-vec documentation](https://docs.rs/orx-concurrent-vec/badge.svg)](https://docs.rs/orx-concurrent-vec)

A thread-safe, efficient and lock-free vector allowing concurrent grow, read and update operations.

## Safe Concurrent Grow & Read & Update

[`ConcurrentVec`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html) provides safe api for concurrent **grow** & **read** & **update** operations (see also [GrowReadUpdate.md](https://github.com/orxfun/orx-concurrent-vec/blob/main/docs/GrowReadUpdate.md) and examples [metrics_collection](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/metrics_collection.rs), [random_actions](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/random_actions.rs) and [concurrent_grow_read_update](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/concurrent_grow_read_update.rs)).

### ① Grow

ConcurrentVec is designed having safe & high performance growth in focus. Elements can be added to the vector using [`push`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.push) and [`extend`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.extend) methods.

```rust ignore
let idx = vec.push("foo".to_string());

// extend guarantees that all elements of the iterator will be written consecutively
let begin_idx = vec.extend((0..42).map(|i| i.to_string()));
```

These methods return the positions in the vector that the values are written to, since otherwise it is not trivially possible to get this information in a concurrent program. For similar reasons, there exist also [`push_for_idx`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.push_for_idx) and [`extend_for_idx`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.extend_for_idx) variants.

### ② Read

In order to prevent race conditions, safe methods of the concurrent vec do not return `&T` or `&mut T` references. Instead, `vec.get(i)` and `vec[i]` provides a reference to the i-th [`ConcurrentElement`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentElement.html) of the vector. A concurrent element then provides us with thread-safe read and write access to the underlying value.

For instance, we can use the value of the i-th element of the concurrent vec as follows:

```rust ignore
vec[i].map(|x| println!("{}", x));  // just do something with the value
let double = vec[i].map(|x| x * 2); // or map it to another value
```

When possible and makes sense, we might as well get the clone or copy of the value:

```rust ignore
let clone: Option<String> = vec.get_cloned(i); // or: vec[i].cloned();
```

#### Read via Iterators

Similar to *iter* on a slice, concurrent vec's [`iter`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.iter) provides an iterator yielding references to concurrent elements of the vec.

```rust ignore
let total_num_characters: usize = vec.iter().map(|elem| elem.map(|x| x.len())).sum();
let students = vec.iter().filter(|elem| elem.map(|person| person.is_student()));
```

Notice that the iterator yields `&ConcurrentElement`, and then we need to use its thread-safe methods (*map* here) to do something with the values. For common use cases like these, there exist shorthands such as:

```rust ignore
let ages = vec.map(|student| student.age());
let total_num_characters: usize = vec.fold(0, |agg, x| agg + x.len());
let students = vec.filter(|person| person.is_student());
```

### ③ Update

Since `ConcurrentElement` provides both read and write access, *get* (or access via index) suffices and *get_mut* is not required.

We can update the value of an element depending on its previous value.

```rust ignore
vec[i].update(|x| {
    match *x < 100 {
        true => *x += 1,
        false => *x = 0,
    }
});
```

Alternatively, we can set or replace its value.

```rust ignore
vec[i].set(String::from("foo"));

let old_value = vec[i].replace(String::from("bar"));
assert_eq!(old_value.as_str(), "foo");
```

#### Updating via Iterators

We do not need *iter_mut* since `ConcurrentElement` allows mutating the elements.

```rust ignore
// double all values
for elem in vec.iter() {
    elem.update(|x| *x *= 2);
}

// set all values to 42
vec.iter().for_each(|elem| elem.set(42));

// set all values to 42 and collect the old ones
let old_values: Vec<_> = vec.iter().map(|elem| elem.replace(42)).collect();
```

## Concurrent Slices

Similar to the standard vec, a concurrent vec can be sliced into concurrent slices using [`slice(range)`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.slice) method. Resulting slices can of course be further sliced. A [`ConcurrentSlice`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentSlice.html) has all functionalities of the concurrent vec except for the growth methods.

```rust ignore
use orx_concurrent_vec::*;

let vec: ConcurrentVec<_> = (0..42).into_iter().collect();

let a = vec.slice(..21);
let b = vec.slice(21..);

// or
let (a, b) = vec.split_at(21);

assert_eq!(a.len(), 21);
assert_eq!(b[0].copied(), 21);
```

Slices are useful in a concurrent program to limit the access of certain actors to a particular region of the data. Methods such as [`split_at`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.split_at) and [`chunks`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.chunks) are particularly helpful for this purpose.

## (Partially) Unsafe Api

`ConcurrentVec` aims to provide an extensive set of vec functionalities while providing thread-safety via access through the concurrent element. However, it also provides unsafe methods to provide references to the elements. These methods and references can be used safely under certain circumstances.

A common scenario where we do not need the checked access occurs when we use **grow** and **read** methods, but not **update** methods. In such a case, we can directly access the values through `&T` references or hold on to these references (or pointers) throughout the lifetime of the vector. This is safe due to the following:
* the concurrent vector will never allow access until the element is completely initialized, and
* the references will remain valid as long as the vec is not dropped due to the pinned element guarantees of the underlying [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) storage.

References and pointers can be obtained using [`get_ref`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.get_ref), [`get_mut`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.get_mut), [`get_raw`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.get_raw), [`get_raw_mut`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.get_raw_mut), [`iter_ref`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.iter_ref) and [`iter_mut`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.iter_mut) methods.

Details can be read in [GrowRead.md](https://github.com/orxfun/orx-concurrent-vec/blob/main/docs/GrowRead.md) and an example safe usage can be found in [concurrent_grow_read_noupdate](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/concurrent_grow_read_noupdate.rs).

## Current Limitations

Currently, `ConcurrentVec` cannot change positions of existing elements concurrently, [`swap`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentVec.html#method.swap) being the only exception:
* `clear` requires a `&mut self` reference.
* methods such as `remove`, `insert` and `pop` are not yet implemented.

## Performance

### Impact of Lock-Free

We can replace `ConcurrentVec<T>` with `Arc<Mutex<Vec<T>>>` which would provide us with entire functionality of the standard vector. However, especially in performance critical scenarios, locking an entire vector for each access might not be a good strategy.

The [updater_reader](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/updater_reader.rs) example aims to demonstrate the impact of locking in such a scenario. In the example, we create and fill a vector and share it with two types of actors, updaters & readers:
* We spawn `num_updaters` threads, each of which continuously draws an index, and updates the element at the given index.
* We spawn `num_readers` threads, each of which continuously draws an index, and reads the value of the element at the given index.

All threads run for a pre-set amount of time. At the end of the experiment, we analyze the number of read and update operations we could perform in the allowed duration.

```bash
cargo run --release --example updater_reader -- --help
cargo run --release --example updater_reader -- --len=10000 --num-readers=8 --num-updaters=8 --duration-seconds=10
```

In the benchmark, we fix the number of updater threads to 4 and change the number of reader threads between 2 and 16.
* With 2 reader threads, we observe that lock-free ConcurrentVec is able to perform four times more operations than arc-mutex-vec.
* As the number of reader threads increases, total number of operations we manage to perform actually decreases with arc-mutex-vec. The heavier the load, the more drastic the impact of locks.
* With ConcurrentVec, on the other hand, number of operations increases as we add more readers. Further, the relation is close to linear. In other words, the benefit of adding a new reader thread remains close to constant.

<img src="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_updater_reader.PNG" alt="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_updater_reader.PNG" />

### Growth Performance

The following experiments focus only on the concurrent growth or collection of the elements.

#### Growth with ***push***

In the first part, *rayon*'s parallel iterator, and push methods of *AppendOnlyVec*, *boxcar::Vec* and *ConcurrentVec* are used to collect results from multiple threads. Further, different underlying pinned vectors of the *ConcurrentVec* are evaluated. You may find the details of the benchmarks at [benches/collect_with_push.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/collect_with_push.rs).

<img src="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_collect_with_push.PNG" alt="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_collect_with_push.PNG" />

 We observe that:
* The default `Doubling` growth strategy leads to efficient concurrent collection of results. Note that this variant does not require any input to construct.
* On the other hand, `Linear` growth strategy performs significantly better. Note that value of this argument means that each fragment of the underlying `SplitVec` will have a capacity of 2^12 (4096) elements. The underlying reason of improvement is potentially be due to less waste and could be preferred with minor knowledge of the data to be pushed.
* Finally, `Fixed` growth strategy is the least flexible and requires perfect knowledge about the hard-constrained capacity (will panic if we exceed). Since it does not outperform `Linear`, we do not necessarily prefer `Fixed` even if we have the perfect knowledge.

The performance can further be improved by using `extend` method instead of `push`. You may see results in the next subsection and details in the [performance notes](https://docs.rs/orx-concurrent-bag/2.3.0/orx_concurrent_bag/#section-performance-notes) of `ConcurrentBag` which has similar characteristics.

#### Growth with ***extend***

The only difference in this follow up experiment is that we use `extend` rather than `push` with *ConcurrentVec*. You may find the details of the benchmarks at [benches/collect_with_extend.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/collect_with_extend.rs).

The expectation is that this approach will solve the performance degradation due to false sharing, which turns out to be true:
* Extending rather than pushing might double the growth performance.
* There is not a significant difference between extending by batches of 64 elements or batches of 65536 elements. We do not need a well tuned number, a large enough batch size seems to be just fine.
* Not all scenarios allow to extend in batches; however, the significant performance improvement makes it preferable whenever possible.

<img src="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_collect_with_extend.PNG" alt="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_collect_with_extend.PNG" />

## Contributing

Contributions are welcome! If you notice an error, have a question or think something could be added or improved, please open an [issue](https://github.com/orxfun/orx-concurrent-vec/issues/new) or create a PR.

## License

This library is licensed under MIT license. See LICENSE for details.
