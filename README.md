# orx-concurrent-vec

[![orx-concurrent-vec crate](https://img.shields.io/crates/v/orx-concurrent-vec.svg)](https://crates.io/crates/orx-concurrent-vec)
[![orx-concurrent-vec documentation](https://docs.rs/orx-concurrent-vec/badge.svg)](https://docs.rs/orx-concurrent-vec)

An efficient, convenient and lightweight grow-only read & write concurrent data structure allowing high performance concurrent collection.

* **convenient**: `ConcurrentVec` can safely be shared among threads simply as a shared reference. It is a [`PinnedConcurrentCol`](https://crates.io/crates/orx-pinned-concurrent-col) with a special concurrent state implementation. Underlying [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) and concurrent bag can be converted back and forth to each other.
* **efficient**: `ConcurrentVec` is a lock free structure suitable for concurrent, copy-free and high performance growth. You may see <a href="#section-benchmarks">benchmarks</a> and further <a href="#section-performance-notes">performance notes</a> for details.

## Examples

Underlying `PinnedVec` guarantees make it straightforward to safely grow with a shared reference which leads to a convenient api as demonstrated below.

The following example demonstrates use of two collections together:
* A `ConcurrentVec` is used to collect measurements taken in random intervals.
  * Concurrent vec is used since while collecting measurements, another thread will be reading them to compute statistics (read & write).
* A `ConcurrentBag` is used to collect statistics from the measurements at defined time intervals.
  * Concurrent bag is used since we do not need to read the statistics until the process completes (write-only).

```rust
use orx_concurrent_vec::*;
use orx_concurrent_bag::*;
use std::time::Duration;

#[derive(Debug, Default)]
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

    fn average(&self) -> i32 {
        match self.count {
            0 => 0,
            _ => self.sum / self.count,
        }
    }
}

// record measurements in random intervals, roughly every 2ms (read & write -> ConcurrentVec)
let measurements = ConcurrentVec::new();
let rf_measurements = &measurements; // just &self to share among threads

// collect metrics every 100 milliseconds (only write -> ConcurrentBag)
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

    // thread to collect metrics every 100 milliseconds
    s.spawn(move || {
        for _ in 0..10 {
            // safely read from measurements vec to compute the metric
            let metric = rf_measurements
                .iter()
                .fold(Metric::default(), |x, value| x.aggregate(value));

            // push result to metrics bag
            rf_metrics.push(metric);

            std::thread::sleep(Duration::from_millis(100));
        }
    });
});

let measurements: Vec<_> = measurements
    .into_inner()
    .into_iter()
    .collect();
dbg!(&measurements);

let averages: Vec<_> = metrics.into_inner().iter().map(|x| x.average()).collect();
println!("averages = {:?}", &averages);

assert_eq!(measurements.len(), 100);
assert_eq!(averages.len(), 10);
```

### Construction

`ConcurrentVec` can be constructed by wrapping any pinned vector implementing `IntoConcurrentPinnedVec`.
Likewise, a concurrent vector can be unwrapped to get the underlying pinned vector with `into_inner` method.

Further, there exist `with_` methods to directly construct the concurrent bag with common pinned vector implementations.

```rust
use orx_concurrent_vec::*;

// default pinned vector -> SplitVec<T, Doubling>
let con_vec: ConcurrentVec<char> = ConcurrentVec::new();
let con_vec: ConcurrentVec<char> = Default::default();
let con_vec: ConcurrentVec<char> = ConcurrentVec::with_doubling_growth();
let con_vec: ConcurrentVec<char, SplitVec<char, Doubling>> = ConcurrentVec::with_doubling_growth();

let con_vec: ConcurrentVec<char> = SplitVec::new().into();
let con_vec: ConcurrentVec<char, SplitVec<char, Doubling>> = SplitVec::new().into();

// SplitVec with [Linear](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Linear.html) growth
// each fragment will have capacity 2^10 = 1024
// and the split vector can grow up to 32 fragments
let con_vec: ConcurrentVec<char, SplitVec<char, Linear>> = ConcurrentVec::with_linear_growth(10, 32);
let con_vec: ConcurrentVec<char, SplitVec<char, Linear>> = SplitVec::with_linear_growth_and_fragments_capacity(10, 32).into();

// [FixedVec](https://docs.rs/orx-fixed-vec/latest/orx_fixed_vec/) with fixed capacity.
// Fixed vector cannot grow; hence, pushing the 1025-th element to this concurrent vector will cause a panic!
let con_vec: ConcurrentVec<char, FixedVec<char>> = ConcurrentVec::with_fixed_capacity(1024);
let con_vec: ConcurrentVec<char, FixedVec<char>> = FixedVec::new(1024).into();
```

Of course, the pinned vector to be wrapped does not need to be empty.

```rust
use orx_concurrent_vec::*;

let split_vec: SplitVec<i32> = (0..1024).collect();
let con_vec: ConcurrentVec<_> = split_vec.into();
```

## Concurrent State and Properties

The concurrent state is modeled simply by an atomic length. Combination of this state and `PinnedConcurrentCol` leads to the following properties:
* Writing to a position of the collection does not block other writes, multiple writes can happen concurrently.
* Each position is written exactly once.
* ⟹ no write & write race condition exists.
* Only one growth can happen at a given time. Growth is copy-free and does not change memory locations of already pushed elements.
* Underlying pinned vector is always valid and can be taken out any time by `into_inner(self)`.
* Attempting to read from a position while its value is being written will yield `None` and will be omitted. If the element has been properly initialized, the other threads will safely read `Some(T)`.
* In other words, a position will be written exactly once, but can be read multiple times concurrently. However, reading can only happen after it is completely written.


## Concurrent Friend Collections

||[`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag)|[`ConcurrentVec`](https://crates.io/crates/orx-concurrent-vec)|[`ConcurrentOrderedBag`](https://crates.io/crates/orx-concurrent-ordered-bag)|
|---|---|---|---|
| Write | Guarantees that each element is written exactly once via `push` or `extend` methods | Guarantees that each element is written exactly once via `push` or `extend` methods | Different in two ways. First, a position can be written multiple times. Second, an arbitrary element of the bag can be written at any time at any order using `set_value` and `set_values` methods. This provides a great flexibility while moving the safety responsibility to the caller; hence, the set methods are `unsafe`. |
| Read | Mainly, a write-only collection. Concurrent reading of already pushed elements is through `unsafe` `get` and `iter` methods. The caller is required to avoid race conditions. | A write-and-read collection. Already pushed elements can safely be read through `get` and `iter` methods. | Not supported currently. Due to the flexible but unsafe nature of write operations, it is difficult to provide required safety guarantees as a caller. |
| Ordering of Elements | Since write operations are through adding elements to the end of the pinned vector via `push` and `extend`, two multi-threaded executions of a code that collects elements into the bag might result in the elements being collected in different orders. | Since write operations are through adding elements to the end of the pinned vector via `push` and `extend`, two multi-threaded executions of a code that collects elements into the bag might result in the elements being collected in different orders. | This is the main goal of this collection, allowing to collect elements concurrently and in the correct order. Although this does not seem trivial; it can be achieved almost trivially when `ConcurrentOrderedBag` is used together with a [`ConcurrentIter`](https://crates.io/crates/orx-concurrent-iter). |
| `into_inner` | Once the concurrent collection is completed, the bag can safely be converted to its underlying `PinnedVec<T>`. | Once the concurrent collection is completed, the bag can safely be converted to its underlying `PinnedVec<T>`. | Growing through flexible setters allowing to write to any position, `ConcurrentOrderedBag` has the risk of containing gaps. `into_inner` call provides some useful metrics such as whether the number of elements pushed elements match the  maximum index of the vector; however, it cannot guarantee that the bag is gap-free. The caller is required to take responsibility to unwrap to get the underlying `PinnedVec<T>` through an `unsafe` call. |
|||||

<div id="section-benchmarks"></div>

## Benchmarks

*You may find the details of the benchmarks at [benches/collect_with_push.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/collect_with_push.rs) and [benches/collect_with_extend.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/collect_with_extend.rs).*

Please see results of benchmarks in the concurrent bag documentation [here](https://docs.rs/orx-concurrent-bag/latest/orx_concurrent_bag/#section-benchmarks). In summary:

* Concurrent bag and concurrent vec provide a high performance collection.
* Importantly, using `extend` rather than `push` provides further significant performance improvements as it allows to avoid a problem known as *false sharing*. Please see the relevant section [here](https://docs.rs/orx-concurrent-bag/latest/orx_concurrent_bag/#section-performance-notes) for details.

## Contributing

Contributions are welcome! If you notice an error, have a question or think something could be improved, please open an [issue](https://github.com/orxfun/orx-concurrent-vec/issues/new) or create a PR.

## License

This library is licensed under MIT license. See LICENSE for details.
