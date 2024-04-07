# orx-concurrent-vec

[![orx-concurrent-vec crate](https://img.shields.io/crates/v/orx-concurrent-vec.svg)](https://crates.io/crates/orx-concurrent-vec)
[![orx-concurrent-vec documentation](https://docs.rs/orx-concurrent-vec/badge.svg)](https://docs.rs/orx-concurrent-vec)

An efficient, convenient and lightweight grow-only and read-and-write concurrent collection.

* **convenient**: `ConcurrentVec` can safely be shared among threads simply as a shared reference. Further, it is a wrapper around any [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) implementation adding concurrent safety guarantees. Underlying pinned vector of `Option<T>` and concurrent vec of `T` can be converted to each other back and forth without any cost (see <a href="#section-construction-and-conversions">construction and conversions</a>).
* **lightweight**: This crate takes a simplistic approach built on pinned vector guarantees which leads to concurrent programs with few dependencies and small binaries (see <a href="#section-approach-and-safety">approach and safety</a> for details).
* **efficient**: `ConcurrentBag` is a grow only collection with immutable elements making use of atomic primitives. This leads to efficient growth and lock-free reading. You may see the details in <a href="#section-benchmarks">benchmarks</a> and further <a href="#section-performance-notes">performance notes</a>.

Note that `ConcurrentVec` is a **read and write** collection. When we only need to collect results concurrently without reading, [`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag) is preferable due to its performance optimizations. Having almost identical api, switching between `ConcurrentVec` and `ConcurrentBag` is straightforward.

# A. Examples

The main feature of `ConcurrentVec` compared to concurrent bag is to enable safe reading while providing efficient growth. It is convenient to share the concurrent vector among threads. `std::sync::Arc` can be used; however, it is not required as demonstrated below.

```rust
use orx_concurrent_vec::*;
use orx_concurrent_bag::*;
use std::time::Duration;

#[derive(Default, Debug)]
struct Metric {
    sum: i32,
    count: i32,
}
impl Metric {
    fn aggregate(self, value: &i32) -> Self {
        Self {
            sum: self.sum + value,
            count: self.count + 1,
        }
    }
}

// record measurements in random intervals (read & write -> ConcurrentVec)
let measurements = ConcurrentVec::new();
let rf_measurements = &measurements; // just &self to share among threads

// collect metrics every 50 milliseconds (only write -> ConcurrentBag)
let metrics = ConcurrentBag::new();
let rf_metrics = &metrics; // just &self to share among threads

std::thread::scope(|s| {
    // thread to store measurements as they arrive
    s.spawn(move || {
        for i in 0..100 {
            std::thread::sleep(Duration::from_millis(i % 5));

            // collect measurements and push to measurements vec
            // simply by calling `push`
            rf_measurements.push(i as i32);
        }
    });

    // thread to collect metrics every 50 milliseconds
    s.spawn(move || {
        for _ in 0..10 {
            // safely read from measurements vec to compute the metric
            let metric = rf_measurements
                .iter()
                .fold(Metric::default(), |x, value| x.aggregate(value));

            // push result to metrics bag
            rf_metrics.push(metric);

            std::thread::sleep(Duration::from_millis(50));
        }
    });

    // thread to print out the values to the stdout every 100 milliseconds
    s.spawn(move || {
        let mut idx = 0;
        loop {
            let current_len = rf_measurements.len_exact();
            let begin = idx;

            for i in begin..current_len {
                // safely read from measurements vec
                if let Some(value) = rf_measurements.get(i) {
                    println!("[{}] = {:?}", i, value);
                    idx += 1;
                } else {
                    idx = i;
                    break;
                }
            }

            if current_len == 100 {
                break;
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    });
});

assert_eq!(measurements.len(), 100);
assert_eq!(metrics.len(), 10);
```

<div id="section-approach-and-safety"></div>

# B. Approach and Safety

`ConcurrentVec` is a wrapper around the `ConcurrentBag` with a focus to enable safe concurrent reading while concurrent bag focuses on efficient growth. Details of the approach and growth related safety guarantees can be found [here](https://docs.rs/orx-concurrent-bag/latest/orx_concurrent_bag/#section-approach-and-safety).

In order to add safe concurrent reading feature to the concurrent bag, the following simple approach is taken:
* When `ConcurrentBag` allocates to grow, it does not initialize the memory of positions which are less than the length (as common in dynamic size vectors). Since reading from a concurrent bag only happens after the consuming `into_inner` method, this is perfectly fine. Read methods such as `get(index)` and `iter()` are available; however, unsafe, due to the possibility of the following sequence of events:
  * `bag.push('a')` is called from thread#1.
  * `bag` atomically increases the `len` to 1.
  * thread#2 calls `bag.get(0)` which is now in bounds.
  * thread#2 receives uninitialized value which is an undefined behavior (UB).
  * thread#1 completes writing `'a'` to the 0-th position (one moment too late).
* `ConcurrentVec` prevents this as follows:
  * Instead of `T`, elements of a concurrent vector are stored as `Option<T>`.
  * When `ConcurrentVec` allocates, it initializes all new positions as `None`.
* Read methods such as `get` or `iter` skips in-bounds but `None` elements. These are the elements which are reserved by some threads; however, the process of writing them is not completed yet.
* Further, elements already pushed to the concurrent vector are immutable. In other words, if the value at a position is of `Some` variant, it will never change.
* Therefore, it is possible to read already pushed elements without a race condition.

<div id="section-benchmarks"></div>

# C. Benchmarks

## Performance with `push`

*You may find the details of the benchmarks at [benches/collect_with_push.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/collect_with_push.rs).*

In the experiment, `rayon`s parallel iterator and `ConcurrentVec`s `push` method are used to collect results from multiple threads.

```rust ignore
// reserve and push one position at a time
for j in 0..num_items_per_thread {
    bag_ref.push(i * 1000 + j);
}
```

* We observe that rayon is significantly faster when the output is very small (`i32` in this experiment).
* As the output gets larger and copies become costlier (`[i32; 32]` here), `ConcurrentVec::push` starts to perform equivalent to or faster than rayon.
* Among the `ConcurrentVec` variants, `Linear` and `Fixed` variants perform faster than the `Doubling` variant:
  * Here we observe the cost of memory initialization immediately on allocation. Since `Doubling` variant allocates more, initialization has a greater impact.
  * `ConcurrentBag` does not perform this operation and it leads to a very high performance concurrent collection. Further, the impact of the underlying pinned vector type is insignificant. Therefore, it is a better choice when we only write results concurrently.
  * The main goal of `ConcurrentVec`, on the other hand, is to enable safe reading while the vector concurrently grows, and it must be preferred in these situations over making unsafe calls.
  * Having almost identical api, switching between bag and vec is straightforward.

The issue leading to poor performance in the *small data & little work* situation can be avoided by using `extend` method in such cases. You may see its impact in the succeeding subsections and related reasons in the <a href="#section-performance-notes">performance notes</a>.

<img src="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_collect_with_push.PNG" alt="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_collect_with_push.PNG" />

## Performance of `extend`

*You may find the details of the benchmarks at [benches/collect_with_extend.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/collect_with_extend.rs).*

The only difference in this follow up experiment is that we use `extend` rather than `push` with `ConcurrentVec`. The expectation is that this approach will solve the performance degradation due to false sharing in the *small data & little work* situation.

```rust ignore
// reserve num_items_per_thread positions at a time
// and then push as the iterator yields
let iter = (0..num_items_per_thread).map(|j| i * 100000 + j);
bag_ref.extend(iter);
```

<img src="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_collect_with_extend.PNG" alt="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_collect_with_extend.PNG" />

Note that we do not need to have perfect information on the number of items to be pushed per thread to get the benefits of `extend`, we can simply `step_by`. Extending by `batch_size` elements will already prevent the dramatic performance degradation provided that `batch_size` elements exceed a cache line.

```rust ignore
// reserve batch_size positions at a time
// and then push as the iterator yields
for j in (0..num_items_per_thread).step_by(batch_size) {
    let iter = (j..(j + batch_size)).map(|j| i * 100000 + j);
    bag_ref.extend(iter);
}
```

<div id="section-performance-notes"></div>

# D. Performance Notes

`ConcurrentVec` is an efficient read-and-write collection. However, it is important to avoid false sharing risk which might lead to a significant performance degradation. Details can be read [here](https://docs.rs/orx-concurrent-bag/latest/orx_concurrent_bag/#section-performance-notes).
<div id="section-construction-and-conversions"></div>

# E. `From` | `into_inner`

`ConcurrentVec` can be constructed by wrapping any pinned vector; i.e., `ConcurrentVec<T>` implements `From<P: PinnedVec<Option<T>>>`.
Likewise, a concurrent vector can be unwrapped to get the underlying pinned vector with `into_inner` method.

Further, there exist `with_` methods to directly construct the concurrent bag with common pinned vector implementations.

```rust
use orx_concurrent_vec::*;

// default pinned vector -> SplitVec<Option<T>, Doubling>
let con_vec: ConcurrentVec<char> = ConcurrentVec::new();
let con_vec: ConcurrentVec<char> = Default::default();
let con_vec: ConcurrentVec<char> = ConcurrentVec::with_doubling_growth();
let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Doubling>> = ConcurrentVec::with_doubling_growth();

let con_vec: ConcurrentVec<char> = SplitVec::new().into();
let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Doubling>> = SplitVec::new().into();

// SplitVec with [Linear](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Linear.html) growth
// each fragment will have capacity 2^10 = 1024
// and the split vector can grow up to 32 fragments
let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Linear>> = ConcurrentVec::with_linear_growth(10, 32);
let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Linear>> = SplitVec::with_linear_growth_and_fragments_capacity(10, 32).into();

// [FixedVec](https://docs.rs/orx-fixed-vec/latest/orx_fixed_vec/) with fixed capacity.
// Fixed vector cannot grow; hence, pushing the 1025-th element to this concurrent vector will cause a panic!
let con_vec: ConcurrentVec<char, FixedVec<Option<char>>> = ConcurrentVec::with_fixed_capacity(1024);
let con_vec: ConcurrentVec<char, FixedVec<Option<char>>> = FixedVec::new(1024).into();
```

Of course, the pinned vector to be wrapped does not need to be empty.

```rust
use orx_concurrent_vec::*;

let split_vec: SplitVec<Option<i32>> = (0..1024).map(Some).collect();
let con_vec: ConcurrentVec<_> = split_vec.into();
```

# License

This library is licensed under MIT license. See LICENSE for details.
